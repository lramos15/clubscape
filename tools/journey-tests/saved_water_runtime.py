"""One admitted saved52 execution. No alternate worlds, migrations, grants or retries."""

from __future__ import annotations

import copy
import json
import os
from pathlib import Path
import shutil
import signal
import stat
import subprocess
import threading
import time

import private_checkpoint as PRIVATE
import run as JOURNEY
import saved_water_gate as GATE


ERRORS = (PRIVATE.CheckpointError, JOURNEY.JourneyError, OSError, ValueError,
          KeyError, TypeError, subprocess.TimeoutExpired)
MAX_VIEW = 32 * 1024 * 1024
MIGRATION_SQL = """
BEGIN TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY;
SELECT jsonb_build_object(
  'database', current_database(),
  'other_clients', (SELECT count(*) FROM pg_stat_activity
    WHERE datname=current_database() AND pid<>pg_backend_pid()
      AND backend_type='client backend'),
  'worlds', (SELECT COALESCE(jsonb_agg(
    (to_jsonb(w)-'runtime_random_key') || jsonb_build_object(
      'runtime_random_key_sha256', encode(sha256(runtime_random_key),'hex'))
    ORDER BY world_id), '[]') FROM game_worlds w),
  'characters', (SELECT COALESCE(jsonb_agg(to_jsonb(c) ORDER BY actor_id),'[]')
    FROM game_characters c),
  'accounts', (SELECT encode(sha256(convert_to(COALESCE(
    jsonb_agg(to_jsonb(a) ORDER BY account_id)::text,'[]'),'UTF8')),'hex') FROM accounts a),
  'account_sessions', (SELECT encode(sha256(convert_to(COALESCE(
    jsonb_agg(to_jsonb(a) ORDER BY token_digest)::text,'[]'),'UTF8')),'hex') FROM account_sessions a),
  'game_sessions', (SELECT encode(sha256(convert_to(COALESCE(
    jsonb_agg(to_jsonb(s) ORDER BY account_id)::text,'[]'),'UTF8')),'hex') FROM game_sessions s),
  'commands', (SELECT jsonb_build_object('count',count(*),'sha256',
    encode(sha256(convert_to(COALESCE(jsonb_agg(to_jsonb(p)
      ORDER BY account_id,sequence)::text,'[]'),'UTF8')),'hex')) FROM processed_game_commands p),
  'lifecycle', (SELECT jsonb_build_object('count',count(*),'sha256',
    encode(sha256(convert_to(COALESCE(jsonb_agg(to_jsonb(l)
      ORDER BY account_id,operation_id)::text,'[]'),'UTF8')),'hex')) FROM game_lifecycle_commands l),
  'migrations', (SELECT COALESCE(jsonb_agg(to_jsonb(m)
    ORDER BY world_id,to_artifact),'[]') FROM game_content_migrations m),
  'schema_migrations', (SELECT encode(sha256(convert_to(COALESCE(
    jsonb_agg(to_jsonb(m) ORDER BY version)::text,'[]'),'UTF8')),'hex') FROM _sqlx_migrations m)
);
COMMIT;
"""
QUIESCENCE_SQL = """
BEGIN READ ONLY;
SELECT jsonb_build_object(
  'database', current_database(),
  'other_clients', (SELECT count(*) FROM pg_stat_activity
    WHERE datname=current_database() AND pid<>pg_backend_pid()
      AND backend_type='client backend'),
  'public_tables', (SELECT COALESCE(jsonb_agg(c.relname ORDER BY c.relname),'[]')
    FROM pg_class c JOIN pg_namespace n ON n.oid=c.relnamespace
    WHERE n.nspname='public' AND c.relkind IN ('r','p','S'))
);
COMMIT;
"""
EMPTY_SQL = """
SELECT count(*) FROM pg_class c JOIN pg_namespace n ON n.oid=c.relnamespace
WHERE n.nspname='public' AND c.relkind IN ('r','p','S');
"""


def failure(error, phase):
    return {
        "phase": error.phase if isinstance(error, PRIVATE.CheckpointError) else phase,
        "reason": error.code if isinstance(error, PRIVATE.CheckpointError) else type(error).__name__,
        "automatic_retry": False,
    }


def verify_migration_start(before):
    GATE.require(before["database"] == "clubscape_journey" and before["other_clients"] == 0
                 and len(before["worlds"]) == len(before["characters"]) == 1,
                 "migration_requires_owned_quiescent_single_world")
    world = before["worlds"][0]
    character = before["characters"][0]
    GATE.require(world["world_id"] == GATE.WORLD
                 and world["runtime_artifact_sha256"] == GATE.FROM
                 and world["content_revision"] == GATE.FROM_REVISION
                 and world["lease_owner"] is None and world["lease_expires_at"] is None
                 and GATE.hex_value(world["runtime_random_key_sha256"])
                 and character["world_id"] == GATE.WORLD
                 and character["actor_id"] == GATE.ACTOR
                 and character["account_id"] == GATE.ACCOUNT
                 and character["last_sequence"] == 498,
                 "migration_original_pin_identity_or_released_lease_changed")
    GATE.require(not any(row["to_artifact"] == GATE.TO for row in before["migrations"]),
                 "target_migration_must_not_already_exist")


def verify_migration(before, after):
    verify_migration_start(before)
    GATE.require(after["database"] == "clubscape_journey" and after["other_clients"] == 0
                 and len(after["worlds"]) == len(after["characters"]) == 1,
                 "migration_requires_owned_quiescent_single_world")
    world = before["worlds"][0]
    old_audits = before["migrations"]
    new_audits = [row for row in after["migrations"] if row not in old_audits]
    GATE.require(len(after["migrations"]) == len(old_audits) + 1
                 and all(row in after["migrations"] for row in old_audits)
                 and not any(row["to_artifact"] == GATE.TO for row in old_audits)
                 and len(new_audits) == 1, "migration_audit_not_exactly_one_new_transition")
    audit = new_audits[0]
    GATE.require(set(audit) == {
        "world_id", "from_artifact", "to_artifact", "world_revision", "migrated_at",
    } and audit["world_id"] == GATE.WORLD and audit["from_artifact"] == GATE.FROM
        and audit["to_artifact"] == GATE.TO and audit["world_revision"] == world["revision"] + 1
        and isinstance(audit["migrated_at"], str) and audit["migrated_at"],
        "migration_audit_identity_changed")
    expected = copy.deepcopy(before)
    updated = expected["worlds"][0]
    updated["runtime_artifact_sha256"] = GATE.TO
    updated["content_revision"] = GATE.TO_REVISION
    updated["revision"] += 1
    updated["lease_fence"] += 1
    updated["state"]["content_revision"] = GATE.TO_REVISION
    updated["state"]["revision"] += 1
    expected["characters"][0]["revision"] += 1
    expected["migrations"] = after["migrations"]
    GATE.require(after == expected, "migration_changed_private_state_outside_exact_pin_revision_audit_fence")
    return {
        "status": "proved", "from_artifact": GATE.FROM, "to_artifact": GATE.TO,
        "complete_private_conservation": True, "restored_private_identity_equal": True,
        "tick_rng_receipts_items_bank_equipment_xp_quests_ui_audio_unchanged": True,
        "world_revision_increment": 1, "character_revision_increment": 1,
        "lease_fence_increment": 1, "new_audits": 1, "lease_released": True,
        "private_values_published": False,
    }


def verify_delivery(reservation):
    canonical = reservation.admission.canonical
    inventory = JOURNEY.read_json(GATE.public_file(canonical, GATE.DELIVERY_FILES, GATE.DELIVERY_FILES_HASH))
    GATE.require(inventory["game_root"] == GATE.DELIVERY and len(inventory["files"]) == 40
                 and len({row["path"] for row in inventory["files"]}) == 40,
                 "prepared_public_delivery_inventory_changed")
    root = PRIVATE.project_path(canonical, GATE.DELIVERY)
    total = 0
    for row in inventory["files"]:
        path = GATE.public_file(root, row["path"], row["sha256"])
        GATE.require(path.stat().st_size == row["bytes"], "prepared_public_delivery_size_changed")
        total += row["bytes"]
    GATE.require(total == 14296055 and JOURNEY.game_identity(root) == GATE.TARGET_IDENTITY,
                 "prepared_public_delivery_not_exact_b2")
    return root


def verify_native_report(saved, scenario, trace):
    GATE.require(scenario["status"] == "continued" and scenario["saved_water_remainder_passed"] is True
                 and scenario["full_journey_passed"] is False and scenario["milestone_accepted"] is False
                 and scenario["saved_water_lifecycle_only_entry"] is True
                 and scenario["identity"]["content_artifact"]["uncompressed_sha256"] == GATE.TO
                 and scenario["saved_water_history"]["source_identity"] == saved.scenario["identity"]
                 and scenario["tutorial_edges_passed"] == saved.scenario["tutorial_edges_passed"]
                 and scenario["observation_checks_passed"] == 3
                 and scenario["checks_passed"] > 377
                 and scenario["source_transition"]["multi_artifact_journey"] is True
                 and scenario.get("private_client_checkpoint", {}).get("status") == "captured",
                 "native_remaining_report_not_complete_or_lineage_changed")
    for segment in JOURNEY.SEGMENTS:
        GATE.require(scenario["segments"][segment]["status"] == "passed",
                     "remaining_segment_not_passed")
        if saved.scenario["segments"][segment]["status"] == "passed":
            GATE.require(scenario["segments"][segment] == saved.scenario["segments"][segment],
                         "completed_segment_rewritten")
    result = scenario["saved_water_range"]
    GATE.require(result["status"] == "passed"
                 and (result["successes"], result["burns"]) in {(1, 0), (0, 1)}
                 and result["cooking_xp_tenths"] == 3000 + result["successes"] * 400
                 and result["rng_controlled"] is False
                 and result["old_reward_replayed"] is False and result["ingredient_reacquisition"] is False,
                 "range_must_keep_real_success_or_burn")
    prefix_bytes = verify_trace_prefix(saved.directory / "evidence/scenario.trace.jsonl", trace)
    inputs = []
    records = saved.scenario["trace_records"]
    with trace.open("rb") as new:
        new.seek(prefix_bytes)
        for line in new:
            GATE.require(len(line) <= MAX_VIEW, "trace_record_too_large")
            row = json.loads(line)
            records += 1
            GATE.require(row["index"] == records, "continuation_trace_not_contiguous")
            GATE.require(row["kind"] != "intentional_sequence_probe", "unadmitted_sequence_probe")
            if row["kind"] == "input":
                inputs.append(row["data"])
    GATE.require(records == scenario["trace_records"]
                 and 4 < len(inputs) <= GATE.BOUNDS["new_world_inputs_including_duplicates"]
                 and scenario["input_count"] == saved.scenario["input_count"] + len(inputs)
                 and scenario["saved_water_new_input_count"] == len(inputs),
                 "remaining_only_input_or_trace_budget_changed")
    water = scenario["saved_water_conversion"]
    recovery = scenario["saved_water_postquest_recovery"]
    GATE.require(water["status"] == "passed" and water["converted_buckets"] == 1
                 and water["source_recipe"] == "recipe.water.bucket"
                 and water["extra_items_xp_or_quests"] is False,
                 "exactly_one_source_water_conversion_required")
    regular = [row for row in inputs if row["retry_of_same_operation"] is False]
    duplicates = [row for row in inputs if row["retry_of_same_operation"] is True]
    GATE.require([row["sequence"] for row in regular] == list(range(499, 499 + len(regular)))
                 and len({row["operation_id"] for row in regular}) == len(regular)
                 and len(duplicates) == 4
                 and all(row["operation_id"] == water["acknowledged_operation_id"]
                         and row["sequence"] == water["sequence"] >= 499 for row in duplicates)
                 and any(row["operation_id"] == water["acknowledged_operation_id"]
                         and row["sequence"] == water["sequence"] for row in regular)
                 and recovery["status"] == "passed" and recovery["new_water_receipt_only"] is True
                 and recovery["same_target_logout_login_reconnect_restart"] is True
                 and recovery["replays"] == 4
                 and recovery["replayed_operation_id"] == water["acknowledged_operation_id"],
                 "only_a_new_acknowledged_water_operation_may_be_replayed")


def verify_trace_prefix(original, continuation):
    GATE.require(continuation.lstat().st_size <= PRIVATE.MAX_EVIDENCE
                 and stat.S_ISREG(continuation.lstat().st_mode), "invalid_continuation_trace")
    size = 0
    with original.open("rb") as old, continuation.open("rb") as new:
        while block := old.read(64 * 1024):
            size += len(block)
            GATE.require(size <= PRIVATE.MAX_EVIDENCE and new.read(len(block)) == block,
                         "historical_trace_prefix_changed")
    return size


class NativeDeadline:
    def __init__(self, process, deadline):
        self.process = process
        self.deadline = deadline
        self.expired = threading.Event()
        self.signal_error = None
        self.timer = threading.Timer(max(0, deadline - time.monotonic()), self.expire)

    def expire(self):
        if self.process.poll() is not None:
            return
        self.expired.set()
        self.kill()

    def kill(self):
        try:
            os.killpg(self.process.pid, signal.SIGKILL)
        except ProcessLookupError as error:
            if self.process.poll() is None:
                self.signal_error = error
        except OSError as error:
            self.signal_error = error

    def __enter__(self):
        try:
            self.timer.start()
        except RuntimeError as error:
            self.kill()
            if self.signal_error is not None:
                raise PRIVATE.CheckpointError(
                    "saved_water", f"native_deadline_signal_failed_{self.signal_error.errno}",
                ) from self.signal_error
            raise PRIVATE.CheckpointError("saved_water", "native_deadline_watchdog_unavailable") from error
        return self

    def __exit__(self, _kind, _error, _traceback):
        self.timer.cancel()
        self.timer.join()
        if self.process.poll() is None:
            if time.monotonic() >= self.deadline:
                self.expire()
            elif _kind is not None:
                self.kill()
        if self.signal_error is not None:
            raise PRIVATE.CheckpointError(
                "saved_water", f"native_deadline_signal_failed_{self.signal_error.errno}",
            ) from self.signal_error
        GATE.require(not self.expired.is_set(), "native_gameplay_deadline")


class Execution:
    def __init__(self, reservation, *, started_at=None):
        reservation.verify()
        self.reservation = reservation
        self.root = reservation.root
        self.directory = self.root / ".local/journey-runs" / reservation.run_id
        PRIVATE.private_directory(PRIVATE.project_path(self.root, ".local/journey-runs"))
        PRIVATE.private_directory(self.directory, new=True)
        self.control = self.directory / "control"
        PRIVATE.private_directory(self.control, new=True)
        self.report_path = reservation.directory / "report.json"
        self.owner = f"clubscape-journey-{reservation.run_id}"
        self.env = os.environ.copy()
        for key in ("DATABASE_URL", "CLUBSCAPE_TEST_DATABASE_URL", "CLUBSCAPE_GAME_ROOT", "CLUBSCAPE_WEB_ROOT"):
            self.env.pop(key, None)
        self.env["CLUBSCAPE_BIND"] = "127.0.0.1:0"
        self.env["CLUBSCAPE_BUILD_REVISION"] = reservation.admission.record["executor"]["code_revision"]
        self.env["CLUBSCAPE_GAME_ROOT"] = str(self.directory / "game-root")
        self.started_at = time.monotonic() if started_at is None else started_at
        self.overall_deadline = self.started_at + GATE.BOUNDS["total_seconds"]
        self.deadline = self.overall_deadline - GATE.BOUNDS["preservation_cleanup_seconds"]
        self.server = None
        self.native = None
        self.saved = None
        self.database_attempted = False
        self.restored_view = None
        self.report = {
            "schema_version": 1, "kind": "exact_saved52_water_execution",
            "run_id": reservation.run_id, "status": "blocked",
            "admission_sha256": reservation.admission.sha256,
            "code_revision": self.env["CLUBSCAPE_BUILD_REVISION"],
            "owned_container_name": self.owner, "database_image": JOURNEY.POSTGRES_IMAGE,
            "bounds": GATE.BOUNDS, "server_starts": 0, "restarts": [],
            "restore_outcome": "not_started", "migration_outcome": "not_started",
            "restored_private_identity_equal": False,
            "saved_execution_performed": False, "full_journey_passed": False,
            "milestone_accepted": False, "later_milestone_authorized": False,
            "automatic_retry": False, "automatic_resume": False,
            "gameplay_sql_used": False, "old_reward_replayed": False,
            "source_state_seeded": False, "private_payloads_published": False,
            "cleanup_passed": False, "owned_resources_retained": True,
        }
        self.save()

    def save(self):
        self.report["elapsed_seconds"] = time.monotonic() - self.started_at
        JOURNEY.write_json(self.report_path, self.report)

    def timeout(self, seconds):
        remaining = self.deadline - time.monotonic()
        GATE.require(remaining > 0, "total_execution_deadline")
        return min(seconds, remaining)

    def phase(self, name):
        self.timeout(1)
        self.reservation.verify()
        self.report["current_phase"] = name
        self.save()

    def command(self, phase, arguments, *, directory=None, output=None, stdin=None,
                maximum=PRIVATE.MAX_METADATA, timeout=90, process_group=False):
        directory = directory or self.directory
        PRIVATE.private_command(
            self.root, directory, phase, arguments, output=output or f"{phase}.stdout",
            stdin=stdin, maximum=maximum, timeout=self.timeout(timeout),
            env=self.env, process_group=process_group,
        )

    def sql(self, directory, name, text):
        path = directory / f"{name}.sql"
        with PRIVATE.private_file(path) as stream:
            stream.write(text.encode("ascii"))
        self.command(name, [
            "docker", "exec", "-i", self.report["owned_container_id"],
            "psql", "--quiet", "--no-psqlrc", "--no-password",
            "--host=/var/run/postgresql", "--username=clubscape", "--dbname=clubscape_journey",
            "--tuples-only", "--no-align", "--set=ON_ERROR_STOP=1",
        ], directory=directory, stdin=path, output=f"{name}.json", maximum=MAX_VIEW)
        return PRIVATE.read_json(directory / f"{name}.json", MAX_VIEW, private=True)

    def owner_identity(self, directory, name):
        self.command(name, [
            "docker", "inspect", "--format",
            '{"id":"{{.Id}}","name":"{{.Name}}","labels":{{json .Config.Labels}},'
            '"running":{{.State.Running}}}',
            self.report["owned_container_id"],
        ], directory=directory, output=f"{name}.json", timeout=15)
        value = PRIVATE.read_json(directory / f"{name}.json", private=True)
        GATE.require(value["id"] == self.report["owned_container_id"]
                     and value["name"] == "/" + self.owner and value["running"] is True
                     and value["labels"].get("clubscape.owner") == self.owner
                     and value["labels"].get("clubscape.scope") == "m1-headless-journey",
                     "target_container_is_not_this_exact_owned_database")
        return value

    def prepare_controls(self):
        files = {}
        for original, name in (
            ("private-client.json", "resume-client-checkpoint.json"),
            ("evidence/scenario.json", "resume-report.json"),
            ("evidence/scenario.trace.jsonl", "resume-trace.jsonl"),
        ):
            target = self.control / name
            PRIVATE.copy_file(self.saved.directory / original, target, PRIVATE.MAX_EVIDENCE, secret=True)
            files[name] = PRIVATE.digest(target)
        self.reservation.phase("controls_prepared", {
            "run_id": self.reservation.run_id, "original_inventory_sha256": GATE.INVENTORY,
            "files": files,
        })

    def native_arguments(self, *, preflight=False):
        record = self.reservation.admission.record
        arguments = [
            self.root / record["binaries"]["simulator"]["path"],
            "scenario", "m1_fresh_account", "--continue-saved-water",
            "--source-manifest", GATE.MANIFEST, "--source-root", ".",
            "--report", (self.directory / "scenario.json").relative_to(self.root),
            "--recovery-control-dir", self.control.relative_to(self.root),
            "--resume-client-checkpoint", (self.control / "resume-client-checkpoint.json").relative_to(self.root),
            "--private-checkpoint-file", (self.control / "private-client-checkpoint.json").relative_to(self.root),
            "--saved-water-admission-revision", self.reservation.admission.revision,
            "--saved-water-admission-sha256", self.reservation.admission.sha256,
            "--saved-water-executor", record["executor"]["id"],
            "--max-seconds", str(GATE.BOUNDS["scenario_seconds"]),
            "--max-inputs", str(GATE.BOUNDS["new_world_inputs_including_duplicates"]),
            "--expected-server-build", record["executor"]["code_revision"],
        ]
        if preflight:
            arguments.append("--validate-resume-only")
        else:
            arguments.extend(["--url", self.server.origin])
        return arguments

    def start_native_phase(self, mode):
        self.reservation.phase(f"native_{mode}_started", {
            "run_id": self.reservation.run_id, "mode": mode,
            "admission_sha256": self.reservation.admission.sha256,
        })

    def restore(self):
        self.phase("owned_empty_database_restore")
        self.database_attempted = True
        self.env["DATABASE_URL"] = JOURNEY.start_database(
            self.directory, self.owner, self.report, deadline=self.deadline,
            command_timeout=GATE.BOUNDS["database_command_seconds"],
        )
        self.save()
        self.owner_identity(self.directory, "restore_owner")
        empty = self.sql(self.directory, "empty_database", EMPTY_SQL)
        GATE.require(empty == 0, "restore_target_must_be_verified_empty")
        self.report["restore_outcome"] = "unknown"
        self.report["saved_execution_performed"] = True
        self.reservation.phase("restore_started", {"archive_sha256": GATE.ARCHIVE, "invocations": 1})
        self.save()
        self.command("restore", [
            "docker", "exec", "-i", self.report["owned_container_id"],
            "pg_restore", "--exit-on-error", "--single-transaction", "--no-password",
            "--host=/var/run/postgresql", "--username=clubscape", "--dbname=clubscape_journey",
        ], stdin=self.saved.directory / "world.pgcustom", timeout=GATE.BOUNDS["restore_seconds"])
        with PRIVATE.private_file(self.directory / "identity.sql") as stream:
            stream.write(PRIVATE.IDENTITY_SQL.encode("ascii"))
        restored = PRIVATE.database_identity(
            self.root, self.directory, self.report["owned_container_id"], self.saved.capsule,
            GATE.WORLD, "restored_identity",
            timeout=self.timeout(GATE.BOUNDS["database_command_seconds"]),
        )
        GATE.require(restored == self.saved.identity["database"], "complete_restored_private_identity_not_equal")
        self.restored_view = self.sql(self.directory, "restored_world", MIGRATION_SQL)
        verify_migration_start(self.restored_view)
        self.report["restore_outcome"] = "verified_identical"
        self.report["restored_private_identity_equal"] = True
        self.save()

    def migrate(self):
        self.phase("production_migrate_ui")
        GATE.require(self.report["restored_private_identity_equal"] is True and self.restored_view is not None,
                     "identity_before_migration_required")
        before = self.sql(self.directory, "migration_before", MIGRATION_SQL)
        verify_migration_start(before)
        GATE.require(before == self.restored_view, "restored_world_changed_before_migration")
        self.reservation.phase("migration_started", {
            "from_artifact": GATE.FROM, "to_artifact": GATE.TO, "invocations": 1,
            "complete_restored_private_identity_equal": True,
        })
        self.report["migration_outcome"] = "unknown"
        self.save()
        migrator = self.root / self.reservation.admission.record["binaries"]["migrator"]["path"]
        self.command("migrate_ui", [migrator, "migrate-ui", "--from", GATE.FROM],
                     timeout=GATE.BOUNDS["migration_seconds"], process_group=True)
        after = self.sql(self.directory, "migration_after", MIGRATION_SQL)
        proof = verify_migration(before, after)
        self.reservation.phase("migration_proved", proof)
        self.report["migration_outcome"] = "proved"
        self.report["migration_proof"] = proof
        self.save()

    def check_target(self):
        record = self.reservation.admission.record
        for name, value in record["binaries"].items():
            GATE.public_file(self.root, GATE.BINARY_PATHS[name], value["sha256"],
                             maximum=512 * 1024 * 1024)
        GATE.require(JOURNEY.game_identity(self.directory / "game-root") == GATE.TARGET_IDENTITY,
                     "owned_b2_delivery_changed")

    def start_server(self):
        self.timeout(35)
        self.check_target()
        GATE.require(self.report["server_starts"] < GATE.BOUNDS["server_starts"],
                     "server_start_allowance_exhausted")
        self.report["server_starts"] += 1
        self.save()
        binary = self.root / self.reservation.admission.record["binaries"]["server"]["path"]
        log_path = self.directory / f"server-{self.report['server_starts']}.jsonl"
        with PRIVATE.private_file(log_path):
            pass
        self.timeout(35)
        self.server = JOURNEY.OwnedServer(
            binary, self.env.copy(), log_path,
        )
        self.server.ready(deadline=self.deadline)
        self.save()

    def restart_if_requested(self):
        requests = list(self.control.glob("restart-*.request.json"))
        GATE.require(len(requests) <= 1 and all(path.name == "restart-after_quest.request.json"
                                               for path in requests), "unadmitted_restart_requested")
        if not requests or self.report["restarts"]:
            return
        request = PRIVATE.read_json(requests[0], private=True)
        GATE.require(request["checkpoint"] == "after_quest" and request["origin"] == self.server.origin
                     and request["require_same_isolated_database"] is True
                     and request["require_same_build_and_content"] is True
                     and 499 <= request["next_sequence"] <= 499 + GATE.BOUNDS["new_world_inputs_including_duplicates"],
                     "restart_request_identity_changed")
        old = self.server.process.pid
        GATE.require(self.server.stop(deadline=self.deadline) == 0, "restart_shutdown_not_clean")
        stopped = self.sql(self.directory, "before_restart", MIGRATION_SQL)
        GATE.require(stopped["worlds"][0]["lease_owner"] is None
                     and stopped["worlds"][0]["lease_expires_at"] is None,
                     "restart_lease_not_released")
        self.start_server()
        ack = {
            "schema_version": 1, "request_id": request["request_id"], "checkpoint": "after_quest",
            "status": "restarted", "origin": self.server.origin, "old_server_pid": old,
            "new_server_pid": self.server.process.pid, "same_isolated_database": True,
            "restart_mode": "graceful",
            "server_binary_sha256": self.reservation.admission.record["binaries"]["server"]["sha256"],
            "game_root_identity": GATE.TARGET_IDENTITY,
        }
        JOURNEY.write_json(self.control / "restart-after_quest.ack.json", ack)
        self.report["restarts"].append(ack)
        self.save()

    def gameplay(self):
        self.phase("remaining_water_range_and_postquest")
        self.start_server()
        self.start_native_phase("gameplay")
        arguments = self.native_arguments()
        log_path = self.directory / "native-gameplay.log"
        execution_deadline = self.deadline
        deadline = time.monotonic() + self.timeout(GATE.BOUNDS["scenario_seconds"])
        self.deadline = min(execution_deadline, deadline)
        watchdog = None
        try:
            with PRIVATE.private_file(log_path) as log:
                self.native = subprocess.Popen(
                    [str(value) for value in arguments], cwd=self.root, env=self.env,
                    stdin=subprocess.DEVNULL, stdout=log, stderr=subprocess.STDOUT,
                    start_new_session=True,
                )
                # Restart handling blocks this thread; the process cap must not depend on it.
                watchdog = NativeDeadline(self.native, self.deadline)
                with watchdog:
                    while self.native.poll() is None:
                        GATE.require(time.monotonic() < deadline, "native_gameplay_deadline")
                        GATE.require(log_path.stat().st_size <= PRIVATE.MAX_METADATA, "native_log_limit")
                        self.restart_if_requested()
                        time.sleep(min(0.05, max(0, deadline - time.monotonic())))
                    code = self.native.wait(timeout=0)
        finally:
            self.deadline = execution_deadline
            if watchdog is not None:
                self.report["native_gameplay_deadline_reached"] = watchdog.expired.is_set()
        GATE.require(code == 0, "native_remaining_scenario_failed")
        scenario = PRIVATE.read_json(self.directory / "scenario.json", MAX_VIEW, private=True)
        verify_native_report(self.saved, scenario, self.directory / "scenario.trace.jsonl")
        GATE.require(len(self.report["restarts"]) == 1, "exact_postquest_restart_not_completed")
        self.report["status"] = "continued"
        self.report["remaining_water_range_verified"] = True
        self.report["source_transition"] = scenario["source_transition"]
        self.save()

    def quiesce(self):
        errors = []
        if self.native is not None and self.native.poll() is None:
            try:
                os.killpg(self.native.pid, signal.SIGTERM)
                try:
                    self.native.wait(timeout=max(0, min(10, self.deadline - time.monotonic())))
                except subprocess.TimeoutExpired:
                    os.killpg(self.native.pid, signal.SIGKILL)
                    self.native.wait(timeout=max(0, min(10, self.deadline - time.monotonic())))
                    errors.append({"phase": "native_stop", "reason": "forced_termination"})
            except ERRORS as error:
                errors.append(failure(error, "native_stop"))
        if self.native is not None:
            self.report["native_gameplay_exit_code"] = self.native.poll()
        if self.server is not None:
            try:
                code = self.server.stop(deadline=self.deadline)
                GATE.require(code == 0, "owned_server_exit_not_clean")
            except ERRORS as error:
                errors.append(failure(error, "server_stop"))
        return errors

    def capture(self):
        self.timeout(1)
        self.reservation.verify()
        GATE.require(self.native is None or self.native.poll() is not None, "native_not_reaped")
        GATE.require(self.server is None or self.server.process.poll() is not None, "server_not_reaped")
        base = PRIVATE.project_path(self.root, ".local/journey-checkpoints")
        PRIVATE.private_directory(base)
        directory = base / self.reservation.run_id
        PRIVATE.private_directory(directory, new=True)
        try:
            self.owner_identity(directory, "capture_owner")
            quiet = self.sql(directory, "quiescence_before", QUIESCENCE_SQL)
            GATE.require(quiet["database"] == "clubscape_journey" and quiet["other_clients"] == 0,
                         "snapshot_database_not_quiescent")
            complete_schema = {"game_worlds", "game_characters", "processed_game_commands",
                               "game_content_migrations", "game_lifecycle_commands", "_sqlx_migrations",
                               "accounts", "account_sessions", "game_sessions"} <= set(quiet["public_tables"])
            before = self.sql(directory, "snapshot_before", MIGRATION_SQL) if complete_schema else quiet
            self.command("database_dump", [
                "docker", "exec", self.report["owned_container_id"], "pg_dump", "--no-password",
                "--host=/var/run/postgresql", "--username=clubscape", "--format=custom",
                "--dbname=clubscape_journey",
            ], directory=directory, output="world.pgcustom", maximum=PRIVATE.MAX_ARCHIVE,
                timeout=GATE.BOUNDS["archive_command_seconds"])
            archive = directory / "world.pgcustom"
            with archive.open("rb") as stream:
                GATE.require(stream.read(5) == b"PGDMP", "invalid_new_custom_archive")
            self.command("archive_integrity", [
                "docker", "exec", "-i", self.report["owned_container_id"],
                "pg_restore", "--format=custom", "--file=/dev/null",
            ], directory=directory, stdin=archive, timeout=GATE.BOUNDS["archive_command_seconds"])
            after = self.sql(directory, "snapshot_after", MIGRATION_SQL) if complete_schema else self.sql(
                directory, "quiescence_after", QUIESCENCE_SQL)
            GATE.require(before == after, "database_changed_around_fresh_snapshot")
            PRIVATE.copy_file(self.directory / "postgres-password", directory / "postgres-password", 4096, secret=True)
            PRIVATE.copy_game_root(self.root, self.directory / "game-root",
                                   directory / "game-root", GATE.TARGET_IDENTITY)
            lineage = directory / "lineage"
            PRIVATE.private_directory(lineage, new=True)
            for row in self.saved.inventory["files"] + [
                {"path": "private-inventory.json", "sha256": GATE.INVENTORY},
                {"path": "availability.json", "sha256": PRIVATE.digest(self.saved.directory / "availability.json")},
            ]:
                self.timeout(1)
                target = PRIVATE.project_path(lineage, GATE.relative(row["path"]))
                parent = lineage
                for part in Path(row["path"]).parts[:-1]:
                    parent /= part
                    PRIVATE.private_directory(parent)
                PRIVATE.copy_file(PRIVATE.project_path(self.saved.directory, row["path"]), target,
                                   PRIVATE.MAX_CHECKPOINT, row["sha256"], secret=True)
            client_status = self.copy_current_evidence(directory)
            PRIVATE.write_json(directory / "private-service.json", {
                "schema_version": 1, "kind": "saved_water_execution_configuration",
                "original_config_do_not_publish": {
                    key: self.env[key] for key in PRIVATE.CONFIG_KEYS if key in self.env
                },
                "server_started": self.server is not None,
                "server_start_attempts": self.report["server_starts"],
                "game_root_identity": GATE.TARGET_IDENTITY,
                "migration_outcome": self.report["migration_outcome"],
                "resume_authorized": False, "automatic_restore": False,
            })
            PRIVATE.write_json(directory / "private-orchestrator-before-cleanup.json", self.report)
            PRIVATE.write_json(directory / "private-identity.json", {
                "schema_version": 1, "kind": "saved_water_phase_snapshot_identity",
                "database_archive_sha256": PRIVATE.digest(archive),
                "database_view_sha256": GATE.sha_bytes(json.dumps(before, sort_keys=True).encode()),
                "database_view_kind": "complete_private_world_and_journals" if complete_schema else "pre_restore_catalog",
                "original_private_identity_sha256": GATE.IDENTITY,
                "client_observation": client_status, "migration_outcome": self.report["migration_outcome"],
                "automatic_restore": False, "resume_authorized": False,
            })
            inventory_hash = PRIVATE.seal_inventory(directory)
            PRIVATE.sync_tree(directory)
            available = {
                "schema_version": 1, "kind": "saved_water_phase_checkpoint", "status": "available",
                "snapshot_available": True, "run_id": self.reservation.run_id,
                "directory": str(directory.relative_to(self.root)),
                "database_archive": {"path": "world.pgcustom", "sha256": PRIVATE.digest(archive),
                                     "bytes": archive.stat().st_size},
                "private_inventory_sha256": inventory_hash, "metadata_before_after_equal": True,
                "archive_integrity_verified": True, "client_observation": client_status,
                "migration_outcome": self.report["migration_outcome"],
                "reconciliation_required": True, "automatic_restore": False, "resume_authorized": False,
                "full_journey_passed": False, "milestone_accepted": False,
                "private_payloads_published": False,
            }
            PRIVATE.write_json(directory / "availability.json", available)
            PRIVATE.sync_directory(directory)
            return available
        except ERRORS as error:
            failed = {
                "status": "failed", "snapshot_available": False,
                "directory": str(directory.relative_to(self.root)),
                **failure(error, "fresh_checkpoint"),
                "owned_database_and_credentials_must_be_retained": True,
                "automatic_restore": False, "resume_authorized": False,
            }
            marker = directory / "availability.json"
            try:
                if marker.exists():
                    marker.unlink()
                    PRIVATE.sync_directory(directory)
                PRIVATE.write_json(directory / "availability-failed.json", failed)
            except ERRORS as marker_error:
                failed["failure_marker_error"] = failure(marker_error, "checkpoint_failure_marker")
            return failed

    def copy_current_evidence(self, directory):
        destination = directory / "evidence"
        PRIVATE.private_directory(destination, new=True)
        for path in sorted(self.directory.iterdir()):
            self.timeout(1)
            metadata = path.lstat()
            GATE.require(not stat.S_ISLNK(metadata.st_mode), "symlink_in_owned_run")
            if stat.S_ISREG(metadata.st_mode) and path.name != "postgres-password":
                PRIVATE.copy_file(path, destination / path.name, PRIVATE.MAX_EVIDENCE, secret=True)
        capsule_path = self.control / "private-client-checkpoint.json"
        if not capsule_path.exists():
            GATE.require(self.native is None, "new_native_capsule_missing_database_must_be_retained")
            return "original5e_capsule_only_no_new_client_capture"
        scenario_path = self.directory / "scenario.json"
        scenario = PRIVATE.read_json(scenario_path, MAX_VIEW, private=True)
        status = scenario.get("private_client_checkpoint", {})
        GATE.require(status.get("status") == "captured"
                     and status.get("sha256") == PRIVATE.digest(capsule_path), "fresh_client_capture_invalid")
        PRIVATE.copy_file(capsule_path, directory / "private-client.json", 2 * 1024 * 1024,
                           status["sha256"], secret=True)
        capsule = PRIVATE.read_json(capsule_path, private=True)
        GATE.require(capsule["last_observed_state"] == scenario["last_snapshot"],
                     "new_capsule_observation_mismatch")
        verify_trace_prefix(self.saved.directory / "evidence/scenario.trace.jsonl",
                            self.directory / "scenario.trace.jsonl")
        for segment in JOURNEY.SEGMENTS:
            if self.saved.scenario["segments"][segment]["status"] == "passed":
                GATE.require(scenario["segments"][segment] == self.saved.scenario["segments"][segment],
                             "failed_run_rewrote_completed_segment")
        GATE.require(scenario["tutorial_edges_passed"] == self.saved.scenario["tutorial_edges_passed"]
                     and scenario["observation_checks_passed"] == 3
                     and scenario["checks_passed"] >= 377, "failed_run_lost_historical_checks")
        return capsule["last_observation_origin"]

    def cleanup(self):
        self.timeout(1)
        checkpoint = self.report.get("private_checkpoint", {})
        GATE.require(checkpoint.get("status") == "available" and checkpoint.get("snapshot_available") is True,
                     "cleanup_forbidden_without_fresh_durable_checkpoint")
        self.owner_identity(self.directory, "cleanup_owner")
        container = self.report["owned_container_id"]
        self.command("cleanup_database", ["docker", "rm", "--force", container], timeout=30)
        self.command("cleanup_verify", [
            "docker", "container", "ls", "--all", "--filter", f"id={container}", "--format", "{{.ID}}",
        ], timeout=15)
        GATE.require(not (self.directory / "cleanup_verify.stdout").read_bytes().strip(),
                     "owned_database_removal_not_verified")
        GATE.require(self.directory == self.root / ".local/journey-runs" / self.reservation.run_id
                     and not self.directory.is_symlink(), "unowned_run_cleanup_forbidden")
        shutil.rmtree(self.directory)
        self.report["owned_resources_retained"] = False
        self.report["cleanup_passed"] = True

    def run(self):
        try:
            self.phase("public_delivery_verification")
            delivery = verify_delivery(self.reservation)
            self.saved = GATE.read_saved52(self.reservation)
            self.report["protected_original_read_verified"] = True
            self.prepare_controls()
            PRIVATE.copy_game_root(self.reservation.admission.canonical, delivery,
                                   self.directory / "game-root", GATE.TARGET_IDENTITY)
            self.phase("native_remaining_preflight")
            self.start_native_phase("preflight")
            self.command("native_preflight", self.native_arguments(preflight=True),
                         output="native-preflight.json", timeout=GATE.BOUNDS["native_preflight_seconds"],
                         process_group=True)
            preflight = PRIVATE.read_json(self.directory / "native-preflight.json", private=True)
            GATE.require(preflight["status"] == "validated" and preflight["network_operations"] == 0
                         and preflight["world_inputs"] == 0
                         and preflight["saved_water_boundary"]["next_sequence"] == 499
                         and preflight["saved_water_boundary"]["to_artifact"] == GATE.TO,
                         "native_remaining_preflight_failed")
            self.restore()
            self.migrate()
            self.gameplay()
        except ERRORS as error:
            self.report["first_failure"] = failure(error, self.report.get("current_phase", "setup"))
        finally:
            self.deadline = self.overall_deadline
            errors = self.quiesce()
            if "owned_container_id" in self.report:
                try:
                    self.report["private_checkpoint"] = self.capture()
                except ERRORS as error:
                    self.report["private_checkpoint"] = {
                        "status": "failed", "snapshot_available": False, **failure(error, "checkpoint_setup"),
                    }
                if self.report["private_checkpoint"].get("status") == "available":
                    try:
                        self.cleanup()
                    except ERRORS as error:
                        errors.append(failure(error, "owned_cleanup"))
            elif self.database_attempted:
                errors.append({"phase": "database_start", "reason": "container_identity_unknown_retained"})
            else:
                self.report["private_checkpoint"] = {
                    "status": "not_attempted", "snapshot_available": False,
                    "reason": "no_database_or_restore_started_original_unchanged",
                }
            self.report["cleanup_errors"] = errors
            if time.monotonic() >= self.overall_deadline:
                errors.append({"phase": "execution", "reason": "total_execution_deadline"})
            if errors or not self.report["cleanup_passed"]:
                self.report["status"] = "blocked"
            self.save()
        return self.report


def execute(reservation, *, started_at=None):
    report = Execution(reservation, started_at=started_at).run()
    print(json.dumps({
        "status": report["status"], "report": GATE.OUTPUT + "/report.json",
        "migration_outcome": report["migration_outcome"],
        "private_checkpoint": report.get("private_checkpoint"),
        "owned_resources_retained": report["owned_resources_retained"],
        "cleanup_passed": report["cleanup_passed"], "first_failure": report.get("first_failure"),
        "full_journey_passed": False, "milestone_accepted": False, "automatic_resume": False,
    }))
    return 0 if report["status"] == "continued" and report["cleanup_passed"] else 1

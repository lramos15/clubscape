#!/usr/bin/env python3
"""Explicit owned restoration at a verified tutorial or unsubmitted goblin-loot boundary."""

import argparse
import json
import os
from pathlib import Path
import re
import signal
import subprocess
import time
import uuid

import private_checkpoint as PRIVATE
import run as JOURNEY


ROOT = JOURNEY.ROOT


def verify_checkpoint(relative, expected_archive):
    directory = PRIVATE.project_path(ROOT, relative)
    parts = Path(relative).parts
    PRIVATE.require(len(parts) == 3 and parts[:2] == (".local", "journey-checkpoints")
                    and re.fullmatch(r"[0-9a-f]{16}", parts[2]),
                    "resume", "not_an_owned_checkpoint_directory")
    metadata = directory.lstat()
    PRIVATE.require(directory.is_dir() and metadata.st_uid == os.getuid()
                    and metadata.st_mode & 0o777 == 0o700,
                    "resume", "checkpoint_directory_not_private")
    available = PRIVATE.read_json(directory / "availability.json", private=True)
    PRIVATE.require(available["status"] == "available" and available["snapshot_available"] is True
                    and available["run_id"] == parts[2]
                    and available["database_archive"]["sha256"] == expected_archive,
                    "resume", "checkpoint_not_available")
    inventory_path = directory / "private-inventory.json"
    PRIVATE.require(PRIVATE.digest(inventory_path) == available["private_inventory_sha256"],
                    "resume", "private_inventory_hash_mismatch")
    inventory = PRIVATE.read_json(inventory_path, private=True)
    for record in inventory["files"]:
        path = PRIVATE.project_path(directory, record["path"])
        metadata = path.lstat()
        PRIVATE.require(metadata.st_uid == os.getuid() and metadata.st_mode & 0o777 == 0o600
                        and metadata.st_size == record["bytes"]
                        and PRIVATE.digest(path) == record["sha256"],
                        "resume", "preserved_file_identity_mismatch")
    PRIVATE.require(PRIVATE.digest(directory / "world.pgcustom") == available["database_archive"]["sha256"],
                    "resume", "database_archive_hash_mismatch")
    capsule = PRIVATE.read_json(directory / "private-client.json", private=True)
    original = PRIVATE.read_json(directory / "private-orchestrator-before-cleanup.json", private=True)
    identity = PRIVATE.read_json(directory / "private-identity.json", private=True)
    scenario = PRIVATE.read_json(directory / "evidence/scenario.json", 64 * 1024 * 1024, private=True)
    manifest = JOURNEY.read_json(ROOT / "content/m1/manifest.json")
    PRIVATE.require(available["source_artifact_sha256"] == manifest["compiled_artifact"]["uncompressed_sha256"],
                    "resume", "different_source_artifact")
    after_goblin_kill = (
        len(scenario["tutorial_edges_passed"]) == 70
        and scenario["current_action"] == "lumbridge.actual_goblin_combat"
        and scenario["first_failure"]["reason"] == "Ground-item permission is unavailable or denied"
        and scenario["segments"]["inventory_equipment_bank_shop"]["status"] == "passed"
    )
    PRIVATE.require((0 < len(scenario["tutorial_edges_passed"]) < 70 or after_goblin_kill)
                    and scenario["segments"]["onboarding_recovery"]["status"] == "passed"
                    and scenario["last_snapshot"] == capsule["last_observed_state"],
                    "resume", "unsupported_resume_boundary")
    state = identity["database"]
    PRIVATE.require(state["world"]["last_sequence"] + 1 == capsule["last_observed_state"]["next_sequence"],
                    "resume", "unreconciled_acknowledged_sequence")
    latest = capsule["latest_attempt"]
    PRIVATE.require(latest is not None, "resume", "missing_original_attempt")
    received = latest["observed_response"]
    matched = [row for row in state["selected_actual_receipts"]
               if row["operation_id"] == latest["operation_id"]]
    if received["kind"] == "error_received" and received["http_status"] == 409:
        PRIVATE.require(not matched and latest["sequence"] == state["world"]["last_sequence"] + 1,
                        "resume", "rejected_attempt_has_conflicting_receipt")
    elif received["kind"] == "acknowledgment_received":
        PRIVATE.require(len(matched) == 1 and matched[0]["sequence"] == latest["sequence"],
                        "resume", "acknowledged_attempt_receipt_missing")
    else:
        raise PRIVATE.CheckpointError("resume", "unknown_attempt_requires_separate_reconciliation")
    control = capsule["latest_control_request"]
    if control is not None:
        PRIVATE.require(isinstance(control["observed_http_status"], int)
                        and 200 <= control["observed_http_status"] < 300
                        and control["observed_error"] is None,
                        "resume", "unknown_control_request_requires_separate_reconciliation")
    return directory, available, capsule, original, identity


def execute(args):
    checkpoint, available, capsule, original, identity = verify_checkpoint(args.checkpoint, args.expected_archive_sha256)
    run_id = uuid.uuid4().hex[:16]
    directory = JOURNEY.private_directory(f".local/journey-runs/{run_id}")
    control = directory / "control"
    control.mkdir(mode=0o700)
    restoration = JOURNEY.private_directory(f".local/journey-restores/{run_id}")
    report_path = JOURNEY.project_path(args.report)
    JOURNEY.require(not report_path.exists(), "Resume must not overwrite an earlier report.")
    report_path.parent.mkdir(parents=True, exist_ok=True, mode=0o700)
    evidence = report_path.parent / f"journey-{run_id}"
    evidence.mkdir(mode=0o700)
    owner = f"clubscape-journey-{run_id}"
    report = {
        "schema_version": 1, "kind": "controlled_real_protocol_journey_resume",
        "run_id": run_id, "recorded_at_unix_ms": int(time.time() * 1000),
        "status": "blocked", "full_journey_passed": False, "milestone_accepted": False,
        "source_state_seeded": False, "gameplay_sql_used": False,
        "database_image": JOURNEY.POSTGRES_IMAGE, "owned_container_name": owner,
        "server_entrypoint": original["server_entrypoint"],
        "restarts": list(original["restarts"]), "commands": [],
        "account_lifecycle": original["account_lifecycle"],
        "account_checks_reexecuted": False,
        "unchecked_segments": list(JOURNEY.SEGMENTS),
        "restoration": {
            "explicit_checkpoint": str(checkpoint.relative_to(ROOT)),
            "original_run_id": available["run_id"],
            "archive_sha256": available["database_archive"]["sha256"],
            "private_inventory_sha256": available["private_inventory_sha256"],
            "fresh_owned_database_restored_from_actual_archive": False,
            "source_artifact_repinned": False, "unknown_operation_retried": False,
        },
    }
    server = simulator = None
    env = os.environ.copy()
    try:
        for key in ("CLUBSCAPE_GAME_ROOT", "CLUBSCAPE_WEB_ROOT", "CLUBSCAPE_TEST_DATABASE_URL"):
            env.pop(key, None)
        env["TMPDIR"] = str(JOURNEY.private_directory(".local/journey-build-work"))
        env["CARGO_TARGET_DIR"] = str(JOURNEY.project_path(".local/journey-target"))
        revision = JOURNEY.bounded(["git", "rev-parse", "HEAD"]).stdout.strip()
        report["revision"] = revision
        report["workspace_dirty"] = bool(JOURNEY.bounded(["git", "status", "--porcelain"]).stdout.strip())
        JOURNEY.bounded([
            "cargo", "build", "--quiet", "-p", "clubscape-server", "-p", "clubscape-sim",
            "--features", "clubscape-sim/journey-server",
        ], env=env, timeout=900)
        report["build_performed_by_orchestrator"] = True
        report["cargo_lock_sha256_used_for_build"] = JOURNEY.sha(ROOT / "Cargo.lock")
        target = Path(env["CARGO_TARGET_DIR"]) / "debug"
        binary = target / ("clubscape-journey-server" if report["server_entrypoint"] == "public-api" else "clubscape-server")
        client = target / "clubscape-sim"
        report["binary_sha256"] = {"server": JOURNEY.sha(binary), "simulator": JOURNEY.sha(client)}
        report["current_phase"] = "explicit_private_database_restore"
        env["DATABASE_URL"] = JOURNEY.start_database(directory, owner, report)
        inspection = '{"id":"{{.Id}}","labels":{{json .Config.Labels}}}'
        PRIVATE.private_command(ROOT, restoration, "owner", [
            "docker", "inspect", "--format", inspection, owner,
        ], output="owner.json", timeout=15)
        owned = PRIVATE.read_json(restoration / "owner.json", private=True)
        PRIVATE.require(owned["id"] == report["owned_container_id"]
                        and owned["labels"].get("clubscape.owner") == owner
                        and owned["labels"].get("clubscape.scope") == "m1-headless-journey",
                        "restore", "wrong_container_owner")
        empty_sql = restoration / "empty.sql"
        with PRIVATE.private_file(empty_sql) as stream:
            stream.write(b"SELECT count(*) FROM pg_class c JOIN pg_namespace n ON n.oid=c.relnamespace WHERE n.nspname='public' AND c.relkind IN ('r','p','S');\n")
        PRIVATE.private_command(ROOT, restoration, "empty", [
            "docker", "exec", "-i", owner, "psql", "--no-psqlrc", "--no-password",
            "--host=/var/run/postgresql", "--username=clubscape", "--dbname=clubscape_journey",
            "--tuples-only", "--no-align", "--set=ON_ERROR_STOP=1",
        ], stdin=empty_sql, output="empty.txt")
        PRIVATE.require((restoration / "empty.txt").read_text().strip() == "0",
                        "restore", "target_database_not_empty")
        PRIVATE.private_command(ROOT, restoration, "restore", [
            "docker", "exec", "-i", owner, "pg_restore", "--exit-on-error",
            "--single-transaction", "--clean", "--if-exists", "--no-password",
            "--host=/var/run/postgresql", "--username=clubscape", "--dbname=clubscape_journey",
        ], stdin=checkpoint / "world.pgcustom", output="restore.stdout", timeout=300)
        with PRIVATE.private_file(restoration / "identity.sql") as stream:
            stream.write(PRIVATE.IDENTITY_SQL.encode("ascii"))
        restored = PRIVATE.database_identity(
            ROOT, restoration, owner, capsule, identity["game_root_identity"]["world_id"], "restored_identity",
        )
        PRIVATE.require(restored == identity["database"], "restore", "restored_private_identity_mismatch")
        report["restoration"].update({
            "fresh_owned_database_restored_from_actual_archive": True,
            "actual_world_actor_rng_and_receipts_identical_before_startup": True,
            "private_identity_sha256": PRIVATE.digest(restoration / "restored_identity.json"),
        })
        game_root = directory / "game-root"
        PRIVATE.copy_game_root(ROOT, checkpoint / "game-root", game_root, identity["game_root_identity"])
        report["game_root_identity"] = JOURNEY.game_identity(game_root)
        PRIVATE.require(report["game_root_identity"] == identity["game_root_identity"],
                        "restore", "restored_game_root_identity_mismatch")
        for original_file, name in [
            (checkpoint / "private-client.json", "resume-client-checkpoint.json"),
            (checkpoint / "evidence/scenario.json", "resume-report.json"),
            (checkpoint / "evidence/scenario.trace.jsonl", "resume-trace.jsonl"),
        ]:
            PRIVATE.copy_file(original_file, control / name, PRIVATE.MAX_EVIDENCE, secret=True)
        env["CLUBSCAPE_GAME_ROOT"] = str(game_root)
        env["CLUBSCAPE_BIND"] = "127.0.0.1:0"
        env["CLUBSCAPE_BUILD_REVISION"] = revision
        report["current_phase"] = "strict_product_server_startup"
        JOURNEY.write_json(report_path, report)
        server = JOURNEY.OwnedServer(binary, env.copy(), evidence / "product-server-0.jsonl")
        address = server.ready()
        scenario_path = evidence / "m1-fresh-account.json"
        report["journey_report"] = str(scenario_path.relative_to(ROOT))
        report["current_phase"] = "real_m1_fresh_account_scenario"
        JOURNEY.write_json(report_path, report)
        command = [
            client, "scenario", "m1_fresh_account", "--url", address,
            "--report", scenario_path.relative_to(ROOT),
            "--expected-server-build", revision, "--max-seconds", str(args.max_seconds),
            "--recovery-control-dir", control.relative_to(ROOT),
            "--private-checkpoint-file", (control / "private-client-checkpoint.json").relative_to(ROOT),
            "--resume-client-checkpoint", (control / "resume-client-checkpoint.json").relative_to(ROOT),
        ]
        with (evidence / "simulator.log").open("w", encoding="utf-8") as log:
            simulator = subprocess.Popen([str(value) for value in command], cwd=ROOT, env=env,
                                         stdin=subprocess.DEVNULL, stdout=log, stderr=subprocess.STDOUT)
            deadline = time.monotonic() + args.max_seconds + 60
            handled = {row["checkpoint"] for row in report["restarts"]}
            while simulator.poll() is None:
                JOURNEY.require(time.monotonic() < deadline, "Resumed journey deadline exceeded.")
                for checkpoint_name in ("onboarding", "after_quest"):
                    request_path = control / f"restart-{checkpoint_name}.request.json"
                    if checkpoint_name in handled or not request_path.exists():
                        continue
                    request = JOURNEY.read_json(request_path)
                    JOURNEY.require(request["checkpoint"] == checkpoint_name
                                    and request["origin"] == server.origin
                                    and request["require_same_isolated_database"] is True,
                                    "Invalid resumed owned restart request.")
                    old_pid = server.process.pid
                    JOURNEY.require(server.stop() == 0, "Owned restart was not clean.")
                    JOURNEY.require(JOURNEY.sha(binary) == report["binary_sha256"]["server"]
                                    and JOURNEY.game_identity(game_root) == report["game_root_identity"],
                                    "Resumed binary/source identity changed.")
                    server = JOURNEY.OwnedServer(binary, env.copy(), evidence / f"product-server-{len(handled) + 1}.jsonl")
                    address = server.ready()
                    ack = {
                        "schema_version": 1, "request_id": request["request_id"],
                        "checkpoint": checkpoint_name, "status": "restarted", "origin": address,
                        "old_server_pid": old_pid, "new_server_pid": server.process.pid,
                        "same_isolated_database": True, "restart_mode": "graceful",
                        "server_binary_sha256": report["binary_sha256"]["server"],
                        "game_root_identity": report["game_root_identity"],
                    }
                    JOURNEY.write_json(control / f"restart-{checkpoint_name}.ack.json", ack)
                    report["restarts"].append(ack)
                    handled.add(checkpoint_name)
                    JOURNEY.write_json(report_path, report)
                time.sleep(0.25)
            code = simulator.wait(timeout=5)
            simulator = None
        scenario = JOURNEY.read_json(scenario_path)
        report["scenario"] = scenario
        report["unchecked_segments"] = [name for name in JOURNEY.SEGMENTS if scenario["segments"][name]["status"] != "passed"]
        JOURNEY.require(code == 0 and JOURNEY.full_journey_passed(scenario),
                        f"Resumed real source journey blocked: {scenario.get('first_failure')}")
        JOURNEY.require(len(report["restarts"]) == 2, "Original onboarding and post-quest restarts were not both proved.")
        report["status"] = "passed"
        report["full_journey_passed"] = True
    except (JOURNEY.JourneyError, PRIVATE.CheckpointError, OSError, ValueError, KeyError,
            subprocess.TimeoutExpired) as error:
        report["first_failure"] = {
            "phase": report.get("current_phase", "restore_setup"),
            "reason": JOURNEY.redact(str(error), env) if isinstance(error, JOURNEY.JourneyError)
            else str(error) if isinstance(error, PRIVATE.CheckpointError) else type(error).__name__,
        }
    finally:
        cleanup_errors = []
        if simulator is not None and simulator.poll() is None:
            simulator.terminate()
            try:
                simulator.wait(timeout=10)
            except subprocess.TimeoutExpired:
                simulator.kill()
                simulator.wait(timeout=10)
        if server is not None:
            try:
                JOURNEY.record_server_exit(report, server.stop())
            except (JOURNEY.JourneyError, OSError, subprocess.TimeoutExpired) as error:
                cleanup_errors.append(JOURNEY.redact(str(error), env))
        JOURNEY.preserve_and_cleanup(directory, owner, report, server, cleanup_errors, report_path)
    print(json.dumps({
        "status": report["status"], "full_journey_passed": report["full_journey_passed"],
        "restoration": report["restoration"], "first_failure": report.get("first_failure"),
        "private_checkpoint": report["private_checkpoint"], "cleanup_passed": report["cleanup_passed"],
        "evidence": str(report_path.relative_to(ROOT)), "milestone_accepted": False,
    }))
    return 0 if report["status"] == "passed" and report["cleanup_passed"] else 1


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--checkpoint", required=True)
    parser.add_argument("--expected-archive-sha256", required=True)
    parser.add_argument("--report", required=True)
    parser.add_argument("--max-seconds", type=int, default=5400)
    args = parser.parse_args()
    JOURNEY.require(re.fullmatch(r"[0-9a-f]{64}", args.expected_archive_sha256) is not None,
                    "An explicit expected archive SHA-256 is required.")
    JOURNEY.require(30 <= args.max_seconds <= 7200, "Resume budget must be 30-7200 seconds.")
    signal.signal(signal.SIGTERM, JOURNEY.interrupted)
    signal.signal(signal.SIGINT, JOURNEY.interrupted)
    return execute(args)


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (PRIVATE.CheckpointError, JOURNEY.JourneyError, OSError, ValueError, KeyError) as error:
        print(json.dumps({"status": "blocked", "phase": "resume_preflight",
                          "reason": str(error) if isinstance(error, (PRIVATE.CheckpointError, JOURNEY.JourneyError)) else type(error).__name__,
                          "private_payloads_published": False}))
        raise SystemExit(1)

"""Private owned-database preservation only. This module never restores or runs gameplay."""

from __future__ import annotations

import hashlib
import json
import os
from pathlib import Path
import re
import selectors
import stat
import subprocess
import time
import urllib.parse
import uuid
from journey_contract import full_journey_passed


MAX_METADATA = 8 * 1024 * 1024
MAX_ARCHIVE = 1024 * 1024 * 1024
MAX_GAME_ROOT = 512 * 1024 * 1024
MAX_EVIDENCE = 512 * 1024 * 1024
MAX_CHECKPOINT = 2 * 1024 * 1024 * 1024
CONFIG_KEYS = (
    "DATABASE_URL", "CLUBSCAPE_BIND", "CLUBSCAPE_BUILD_REVISION",
    "CLUBSCAPE_GAME_ROOT", "CLUBSCAPE_WEB_ROOT",
)

IDENTITY_SQL = """
SELECT jsonb_build_object(
  'database', current_database(),
  'server_version_num', current_setting('server_version_num'),
  'world_count', (SELECT count(*) FROM game_worlds),
  'character_count', (SELECT count(*) FROM game_characters),
  'other_clients', (SELECT count(*) FROM pg_stat_activity
    WHERE datname = current_database() AND pid <> pg_backend_pid()
      AND backend_type = 'client backend'),
  'world', (SELECT jsonb_build_object(
    'world_id', w.world_id, 'content_revision', w.content_revision,
    'schema_version', w.schema_version, 'revision', w.revision, 'tick', w.tick,
    'artifact_sha256', w.runtime_artifact_sha256,
    'private_rng_key_sha256', encode(sha256(w.runtime_random_key), 'hex'),
    'world_state_sha256', encode(sha256(convert_to(w.state::text, 'UTF8')), 'hex'),
    'last_tick_receipt_sha256',
      encode(sha256(convert_to(w.last_tick_result::text, 'UTF8')), 'hex'),
    'lease_owner', w.lease_owner, 'lease_fence', w.lease_fence,
    'lease_expires_at', w.lease_expires_at,
    'account_id', c.account_id, 'actor_id', c.actor_id,
    'character_revision', c.revision, 'last_sequence', c.last_sequence,
    'actor_state_sha256',
      encode(sha256(convert_to((w.state->'characters'->c.actor_id)::text, 'UTF8')), 'hex'),
    'login_name', a.login_name,
    'account_sha256', encode(sha256(convert_to(to_jsonb(a)::text, 'UTF8')), 'hex')
    ) FROM game_worlds w
      JOIN game_characters c ON c.world_id = w.world_id
      JOIN accounts a ON a.account_id = c.account_id
    WHERE w.world_id = :'world_id'::uuid AND c.account_id = :'account_id'::uuid),
  'commands', (SELECT jsonb_build_object(
    'count', count(*), 'maximum_sequence', max(sequence),
    'journal_sha256', encode(sha256(convert_to(COALESCE(
      jsonb_agg(jsonb_build_array(operation_id, sequence, intent_version,
        encode(intent_hash, 'hex'), world_revision, character_revision,
        encode(sha256(convert_to(committed_result::text, 'UTF8')), 'hex'),
        committed_at) ORDER BY sequence)::text, '[]'), 'UTF8')), 'hex')
    ) FROM processed_game_commands
    WHERE world_id = :'world_id'::uuid AND account_id = :'account_id'::uuid),
  'selected_actual_receipts', (SELECT COALESCE(
    jsonb_agg(to_jsonb(p) ORDER BY sequence), '[]'::jsonb)
    FROM processed_game_commands p
    WHERE world_id = :'world_id'::uuid AND account_id = :'account_id'::uuid
      AND operation_id = ANY(string_to_array(:'operation_ids', ',')::uuid[])),
  'selected_actual_lifecycle_receipts', (SELECT COALESCE(
    jsonb_agg(to_jsonb(l) ORDER BY operation_id), '[]'::jsonb)
    FROM game_lifecycle_commands l
    WHERE world_id = :'world_id'::uuid AND account_id = :'account_id'::uuid
      AND operation_id = ANY(string_to_array(:'operation_ids', ',')::uuid[])),
  'lifecycle_journal_sha256', (SELECT encode(sha256(convert_to(COALESCE(
    jsonb_agg(to_jsonb(l) ORDER BY operation_id)::text, '[]'), 'UTF8')), 'hex')
    FROM game_lifecycle_commands l
    WHERE world_id = :'world_id'::uuid AND account_id = :'account_id'::uuid)
);
"""


class CheckpointError(RuntimeError):
    def __init__(self, phase, code):
        self.phase = phase
        self.code = code
        super().__init__(f"{phase}: {code}")


def require(condition, phase, code):
    if not condition:
        raise CheckpointError(phase, code)


def digest(path):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def project_path(root, value):
    path = Path(value)
    require(not path.is_absolute() and path.parts
            and all(part not in {".", ".."} for part in path.parts),
            "paths", "non_project_path")
    current = root
    for part in path.parts:
        current /= part
        require(not current.is_symlink(), "paths", "symlink_rejected")
    return current


def private_directory(path, *, new=False):
    path.mkdir(mode=0o700, exist_ok=not new)
    metadata = path.lstat()
    require(stat.S_ISDIR(metadata.st_mode) and metadata.st_uid == os.getuid()
            and stat.S_IMODE(metadata.st_mode) == 0o700,
            "permissions", "directory_must_be_owned_0700")


def private_file(path):
    descriptor = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, 0o600)
    return os.fdopen(descriptor, "wb")


def read_json(path, maximum=MAX_METADATA, *, private=False):
    metadata = path.lstat()
    require(stat.S_ISREG(metadata.st_mode) and metadata.st_size <= maximum,
            "metadata", "invalid_or_oversized_file")
    if private:
        require(metadata.st_uid == os.getuid() and stat.S_IMODE(metadata.st_mode) == 0o600,
                "permissions", "file_must_be_owned_0600")
    with path.open("rb") as stream:
        data = stream.read(maximum + 1)
    require(len(data) <= maximum, "metadata", "file_grew_beyond_bound")
    return json.loads(data)


def write_json(path, value):
    encoded = json.dumps(value, sort_keys=True, indent=2).encode("utf-8") + b"\n"
    require(len(encoded) <= MAX_METADATA, "metadata", "serialized_metadata_too_large")
    with private_file(path) as stream:
        stream.write(encoded)
        stream.flush()
        os.fsync(stream.fileno())


def sync_directory(path):
    descriptor = os.open(path, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW)
    try:
        os.fsync(descriptor)
    finally:
        os.close(descriptor)


def private_command(root, directory, phase, arguments, *, output, maximum=MAX_METADATA,
                    stdin=None, timeout=90):
    """Bound both streams; command errors never disclose captured stdout/stderr."""
    process = None
    input_stream = None
    try:
        if stdin is not None:
            input_stream = stdin.open("rb")
        with private_file(directory / output) as out, \
                private_file(directory / f"{phase}.stderr") as err, \
                selectors.DefaultSelector() as selector:
            process = subprocess.Popen(
                [str(argument) for argument in arguments], cwd=root,
                stdin=input_stream if input_stream else subprocess.DEVNULL,
                stdout=subprocess.PIPE, stderr=subprocess.PIPE,
            )
            selector.register(process.stdout, selectors.EVENT_READ, (out, maximum))
            selector.register(process.stderr, selectors.EVENT_READ, (err, 1024 * 1024))
            sizes = {out: 0, err: 0}
            deadline = time.monotonic() + timeout
            while selector.get_map():
                remaining = deadline - time.monotonic()
                require(remaining > 0, phase, "command_timeout")
                for key, _ in selector.select(min(remaining, 0.25)):
                    data = os.read(key.fileobj.fileno(), 64 * 1024)
                    if not data:
                        selector.unregister(key.fileobj)
                        key.fileobj.close()
                        continue
                    destination, limit = key.data
                    sizes[destination] += len(data)
                    require(sizes[destination] <= limit, phase, "command_output_limit")
                    destination.write(data)
            remaining = deadline - time.monotonic()
            require(remaining > 0, phase, "command_timeout")
            code = process.wait(timeout=remaining)
            for stream in (out, err):
                stream.flush()
                os.fsync(stream.fileno())
            require(code == 0, phase, f"command_exit_{code}")
    except subprocess.TimeoutExpired as error:
        raise CheckpointError(phase, "command_timeout") from error
    except OSError as error:
        raise CheckpointError(phase, f"os_error_{error.errno}") from error
    finally:
        if process is not None:
            if process.poll() is None:
                process.kill()
            process.wait(timeout=10)
            for stream in (process.stdout, process.stderr):
                if stream is not None and not stream.closed:
                    stream.close()
        if input_stream is not None:
            input_stream.close()


def copy_file(source, target, maximum, expected_sha256=None, *, secret=False):
    metadata = source.lstat()
    require(stat.S_ISREG(metadata.st_mode) and metadata.st_size <= maximum,
            "copy", "invalid_or_oversized_source")
    if secret:
        require(metadata.st_uid == os.getuid() and stat.S_IMODE(metadata.st_mode) == 0o600,
                "permissions", "source_secret_must_be_owned_0600")
    with source.open("rb") as original, private_file(target) as destination:
        size = 0
        while block := original.read(64 * 1024):
            size += len(block)
            require(size <= maximum, "copy", "source_grew_beyond_bound")
            destination.write(block)
        destination.flush()
        os.fsync(destination.fileno())
    actual = digest(target)
    require(actual == digest(source), "copy", "source_changed_during_copy")
    if expected_sha256 is not None:
        require(actual == expected_sha256, "copy", "source_hash_mismatch")
    return size


def copy_game_root(root, source, destination, identity):
    source = project_path(root, source.relative_to(root))
    private_directory(destination, new=True)
    descriptor_path = source / "clubscape-game.json"
    assets_path = source / "clubscape-game-assets.json"
    descriptor = read_json(descriptor_path)
    assets = read_json(assets_path)
    require(digest(descriptor_path) == identity["descriptor_sha256"]
            and digest(assets_path) == identity["assets_manifest_sha256"]
            and descriptor["sha256"] == identity["artifact_sha256"]
            and descriptor["world_id"] == identity["world_id"],
            "game_root", "original_pinned_identity_changed")
    files = {
        "clubscape-game.json": identity["descriptor_sha256"],
        "clubscape-game-assets.json": identity["assets_manifest_sha256"],
    }
    for name, expected in [(descriptor["artifact"], descriptor["sha256"]),
                           *((entry["path"], entry["sha256"]) for entry in assets["files"])]:
        require(name not in files or files[name] == expected,
                "game_root", "conflicting_file_identity")
        files[name] = expected
    require(0 < len(files) <= 20003, "game_root", "file_count_limit")
    total = 0
    for name, expected in sorted(files.items()):
        original = project_path(source, name)
        copied = project_path(destination, name)
        parent = destination
        for part in Path(name).parts[:-1]:
            parent /= part
            private_directory(parent)
        total += copy_file(original, copied, MAX_GAME_ROOT - total, expected)
    require(total <= MAX_GAME_ROOT, "game_root", "byte_limit")


def database_identity(root, directory, owner, capsule, world_id, phase):
    account = capsule["private_authentication_do_not_publish"]["account_id"]
    expected_world = str(uuid.UUID(world_id))
    expected_account = str(uuid.UUID(account))
    operation_ids = {
        receipt["operation_id"] for receipt in capsule["original_receipts"].values()
        if receipt is not None
    }
    if capsule["latest_attempt"] is not None:
        operation_ids.add(capsule["latest_attempt"]["operation_id"])
    if capsule["latest_control_request"] is not None:
        operation_ids.add(capsule["latest_control_request"]["operation_id"])
    require(len(operation_ids) <= 4, phase, "operation_identity_limit")
    operation_ids = sorted(str(uuid.UUID(value)) for value in operation_ids)
    command = [
        "docker", "exec", "-i", owner, "psql", "--no-psqlrc", "--no-password",
        "--host=/var/run/postgresql", "--username=clubscape", "--dbname=clubscape_journey",
        "--tuples-only", "--no-align", "--set=ON_ERROR_STOP=1",
        f"--set=world_id={expected_world}", f"--set=account_id={expected_account}",
        "--set=operation_ids=" + ",".join(operation_ids),
    ]
    private_command(root, directory, phase, command, output=f"{phase}.json",
                    stdin=directory / "identity.sql")
    value = read_json(directory / f"{phase}.json", private=True)
    require(value["database"] == "clubscape_journey" and value["other_clients"] == 0
            and value["world_count"] == 1 and value["character_count"] == 1,
            phase, "database_not_owned_and_quiescent")
    world = value["world"]
    require(isinstance(world, dict), phase, "source_world_or_character_missing")
    auth = capsule["private_authentication_do_not_publish"]
    observed = capsule["last_observed_state"]
    require(world["world_id"] == expected_world and world["account_id"] == expected_account
            and world["actor_id"] == capsule["actor_id"]
            and world["login_name"] == auth["login_name"],
            phase, "private_actor_identity_mismatch")
    require(world["artifact_sha256"] == capsule["source_identity"]["content_artifact"]["uncompressed_sha256"]
            and world["content_revision"] == capsule["source_identity"]["content_revision"],
            phase, "source_artifact_mismatch")
    require(world["last_sequence"] >= observed["next_sequence"] - 1
            and world["revision"] >= observed["revision"]
            and world["tick"] >= observed["tick"],
            phase, "acknowledged_progress_missing")
    for field in ("private_rng_key_sha256", "world_state_sha256", "actor_state_sha256",
                  "account_sha256"):
        require(isinstance(world[field], str) and re.fullmatch(r"[0-9a-f]{64}", world[field]),
                phase, "private_state_or_rng_identity_missing")
    return value


def preserve_checkpoint(root, run_directory, report, server):
    """Capture after the simulator/server are reaped, before deleting owned PostgreSQL."""
    from dying_observe import success_checkpoint_eligible
    run_id = report["run_id"]
    require(re.fullmatch(r"[0-9a-f]{16}", run_id), "ownership", "invalid_run_id")
    require(run_directory == root / ".local/journey-runs" / run_id,
            "ownership", "run_directory_mismatch")
    require(server is not None and server.process.poll() is not None,
            "quiesce", "owned_server_not_reaped")
    base = project_path(root, ".local/journey-checkpoints")
    private_directory(base)
    directory = base / run_id
    private_directory(directory, new=True)
    phase = "client_capsule"
    try:
        capsule_path = project_path(root, run_directory.relative_to(root) / "control/private-client-checkpoint.json")
        capsule = read_json(capsule_path, 2 * 1024 * 1024, private=True)
        scenario_path = project_path(root, report["journey_report"])
        scenario = read_json(scenario_path, 64 * 1024 * 1024)
        status = scenario["private_client_checkpoint"]
        require(status["status"] == "captured" and digest(capsule_path) == status["sha256"],
                phase, "client_capsule_not_completed")
        preservable = (scenario["status"] == "blocked"
                       or success_checkpoint_eligible(scenario) or full_journey_passed(scenario))
        require(capsule["schema_version"] == 1 and capsule["kind"] == "private_m1_client_checkpoint"
                and capsule["scenario"] == "m1_fresh_account" and preservable
                and capsule["report_path"] == report["journey_report"]
                and capsule["actor_id"] == scenario["last_snapshot"]["player"]["actor_id"]
                and capsule["private_authentication_do_not_publish"]["account_id"] == scenario["synthetic_account_id"]
                and capsule["source_identity"] == scenario["identity"]
                and capsule["last_observed_state"] == scenario["last_snapshot"]
                and capsule["server_build_revision"] == scenario["server_build_revision"],
                phase, "client_capsule_does_not_match_actual_blocker")
        require(capsule["resume_authorized"] is False and capsule["automatic_restore"] is False,
                phase, "restore_authorization_not_permitted")
        copy_file(capsule_path, directory / "private-client.json", 2 * 1024 * 1024,
                  status["sha256"], secret=True)
        copy_file(run_directory / "postgres-password", directory / "postgres-password",
                  4096, secret=True)
        phase = "ownership"
        owner = report["owned_container_name"]
        require(owner == f"clubscape-journey-{run_id}", phase, "container_name_mismatch")
        inspection = ('{"id":"{{.Id}}","labels":{{json .Config.Labels}},'
                      '"running":{{.State.Running}},"ports":{{json .NetworkSettings.Ports}}}')
        private_command(root, directory, phase, [
            "docker", "inspect", "--format", inspection, owner,
        ], output="container.json", timeout=15)
        container = read_json(directory / "container.json", private=True)
        require(container["id"] == report["owned_container_id"] and container["running"] is True
                and container["labels"].get("clubscape.scope") == "m1-headless-journey"
                and container["labels"].get("clubscape.owner") == owner,
                phase, "container_identity_or_owner_mismatch")
        phase = "service_identity"
        require(digest(server.binary) == report["binary_sha256"]["server"],
                phase, "server_binary_changed")
        database_url = urllib.parse.urlsplit(server.env["DATABASE_URL"])
        password_path = directory / "postgres-password"
        with password_path.open(encoding="ascii") as stream:
            database_password = stream.read(4096).strip()
        require(database_url.scheme == "postgresql" and database_url.hostname == "127.0.0.1"
                and database_url.username == "clubscape"
                and database_url.path == "/clubscape_journey"
                and urllib.parse.unquote(database_url.password or "") == database_password
                and container["ports"]["5432/tcp"] == [{
                    "HostIp": "127.0.0.1", "HostPort": str(database_url.port),
                }],
                phase, "database_configuration_identity_mismatch")
        identity = report["game_root_identity"]
        require(identity["artifact_sha256"] == capsule["source_identity"]["content_artifact"]["uncompressed_sha256"],
                phase, "client_and_service_artifacts_differ")
        write_json(directory / "private-service.json", {
            "schema_version": 1, "run_id": run_id,
            "original_server_pid": server.process.pid,
            "original_server_exit_code": server.process.returncode,
            "original_server_origin": server.origin,
            "original_server_binary": str(server.binary),
            "original_config_do_not_publish": {key: server.env[key] for key in CONFIG_KEYS if key in server.env},
            "server_entrypoint": report["server_entrypoint"],
            "server_binary_sha256": report["binary_sha256"]["server"],
            "database_image": report["database_image"],
            "owned_container": container, "game_root_identity": identity,
            "resume_authorized": False, "automatic_restore": False,
        })
        phase = "game_root"
        copy_game_root(root, Path(server.env["CLUBSCAPE_GAME_ROOT"]),
                       directory / "game-root", identity)
        phase = "evidence"
        private_directory(directory / "evidence", new=True)
        trace_path = project_path(root, capsule["trace_path"])
        require(trace_path == scenario_path.with_suffix(".trace.jsonl"),
                phase, "trace_identity_mismatch")
        copy_file(scenario_path, directory / "evidence/scenario.json", MAX_EVIDENCE)
        copy_file(trace_path, directory / "evidence/scenario.trace.jsonl", MAX_EVIDENCE)
        write_json(directory / "private-orchestrator-before-cleanup.json", report)
        with private_file(directory / "identity.sql") as stream:
            stream.write(IDENTITY_SQL.encode("ascii"))
            stream.flush()
            os.fsync(stream.fileno())
        phase = "database_before"
        before = database_identity(root, directory, owner, capsule, identity["world_id"], phase)
        phase = "database_dump"
        private_command(root, directory, phase, [
            "docker", "exec", owner, "pg_dump", "--no-password",
            "--host=/var/run/postgresql", "--username=clubscape",
            "--format=custom", "--dbname=clubscape_journey",
        ], output="world.pgcustom", maximum=MAX_ARCHIVE, timeout=300)
        archive = directory / "world.pgcustom"
        with archive.open("rb") as stream:
            require(stream.read(5) == b"PGDMP", phase, "invalid_custom_archive_header")
        phase = "archive_integrity"
        private_command(root, directory, phase, [
            "docker", "exec", "-i", owner, "pg_restore", "--format=custom", "--file=/dev/null",
        ], output="archive-check.stdout", stdin=archive, timeout=300)
        phase = "database_after"
        after = database_identity(root, directory, owner, capsule, identity["world_id"], phase)
        require(before == after, phase, "database_changed_around_snapshot")
        write_json(directory / "private-identity.json", {
            "schema_version": 1, "database": before,
            "hash_scope": "Complete persisted world/actor, real private RNG key, account, game/lifecycle journals and selected actual receipts.",
            "client_capsule_sha256": digest(directory / "private-client.json"),
            "service_config_sha256": digest(directory / "private-service.json"),
            "database_archive_sha256": digest(archive),
            "game_root_identity": identity,
            "receipt_state_reconciliation_required": True,
            "restore_executed": False, "resume_authorized": False,
        })
        phase = "inventory"
        files = []
        total = 0
        for path in sorted(directory.rglob("*")):
            metadata = path.lstat()
            require(metadata.st_uid == os.getuid(), phase, "unowned_preserved_path")
            if stat.S_ISDIR(metadata.st_mode):
                require(stat.S_IMODE(metadata.st_mode) == 0o700, phase, "nonprivate_directory")
                continue
            require(stat.S_ISREG(metadata.st_mode) and stat.S_IMODE(metadata.st_mode) == 0o600,
                    phase, "nonprivate_file")
            total += metadata.st_size
            require(total <= MAX_CHECKPOINT, phase, "checkpoint_byte_limit")
            files.append({"path": str(path.relative_to(directory)), "bytes": metadata.st_size,
                          "sha256": digest(path)})
        write_json(directory / "private-inventory.json", {"schema_version": 1, "files": files})
        phase = "durability"
        directories = [path for path in directory.rglob("*") if path.is_dir()]
        for path in sorted(directories, key=lambda value: len(value.parts), reverse=True):
            sync_directory(path)
        sync_directory(directory)
        sync_directory(base)
        sync_directory(base.parent)
        available = {
            "schema_version": 1, "status": "available", "snapshot_available": True,
            "run_id": run_id, "directory": str(directory.relative_to(root)),
            "database_archive": {"path": "world.pgcustom", "bytes": archive.stat().st_size,
                                 "sha256": digest(archive)},
            "private_inventory_sha256": digest(directory / "private-inventory.json"),
            "source_artifact_sha256": identity["artifact_sha256"],
            "captured_server_sha256": report["binary_sha256"]["server"],
            "ownership_and_database_quiescence_verified": True,
            "metadata_before_after_equal": True, "archive_integrity_verified": True,
            "restore_executed": False, "resume_authorized": False,
            "receipt_state_reconciliation_required": True,
            "recoverability": "Requires explicit restore validation and actual receipt/state reconciliation.",
            "private_payloads_published": False, "full_journey_passed": False,
            "captured_scenario_status": scenario["status"],
            "captured_full_journey_passed": full_journey_passed(scenario),
        }
        write_json(directory / "availability.json", available)
        sync_directory(directory)
        return available
    except (CheckpointError, OSError, ValueError, KeyError, TypeError) as error:
        failure = {
            "schema_version": 1, "status": "failed", "snapshot_available": False,
            "recoverable_checkpoint": False, "run_id": run_id,
            "directory": str(directory.relative_to(root)),
            "phase": error.phase if isinstance(error, CheckpointError) else phase,
            "reason": error.code if isinstance(error, CheckpointError)
            else f"os_error_{error.errno}" if isinstance(error, OSError) else "invalid_private_metadata",
            "private_payloads_published": False, "restore_executed": False,
            "resume_authorized": False,
        }
        marker = directory / "availability.json"
        if marker.exists():
            try:
                marker.unlink()
                sync_directory(directory)
            except OSError:
                failure["success_marker_invalidation_failed"] = True
        try:
            write_json(directory / "availability-failed.json", failure)
        except (CheckpointError, OSError):
            failure["availability_manifest_write_failed"] = True
        return failure

#!/usr/bin/env python3
"""One admitted, character-free, real-source water migration operator preflight."""

import argparse
import copy
import json
import os
from pathlib import Path
import re
import signal
import subprocess
import sys
import tempfile
import time
import uuid

import run as journey


AUTHORITY = "milestones/evidence/m1-water-upgrade-preflight-authorization.json"
REVIEW = "milestones/evidence/m1-water-item-on-review.json"
PREPARATION = ".local/water-source-operator-01/preparation.json"
OUTPUT = ".local/water-upgrade-preflight/operator-01"
JOURNAL = ".local/water-upgrade-preflight/operator-authority-v1.json"
WORKTREE = ".worktrees/m1-water-integration"
COUNTS = (
    "accounts", "account_sessions", "game_characters", "game_sessions",
    "processed_game_commands", "game_lifecycle_commands",
)
LEASE_FIELDS = ("lease_owner", "lease_fence", "lease_expires_at")
LIMITS = {
    "source_profiles": 2,
    "source_only_disposable_worlds": 2,
    "source_server_starts": 4,
    "source_operator_invocations": 10,
    "source_operator_timeout_seconds": 35,
    "source_operator_accounts_or_gameplay_commands": 0,
    "database_cpus": 2,
    "database_memory_mib": 512,
    "database_ready_seconds": 60,
    "maximum_concurrent_databases": 1,
    "source_only_asset_limit_mib_per_bundle": 512,
}
BUNDLE_MAX_BYTES = LIMITS["source_only_asset_limit_mib_per_bundle"] * 1024 * 1024
require = journey.require


def validate_admission(authority, review, preparation):
    require(authority["task"] == "M1-WATER-UPGRADE-PREFLIGHT"
            and authority["executor"] == "director"
            and authority["worktree"] == WORKTREE
            and authority["status"] == "admitted_isolated_synthetic_validation_only",
            "The exact Director source-only preflight admission is required.")
    workload = authority["workload"]
    require(all(type(workload[key]) is int and workload[key] == value
                for key, value in LIMITS.items()),
            "The source-only workload does not match this bounded program.")
    require(workload["source_operator_journal"] == f"{WORKTREE}/{JOURNAL}"
            and workload["database"] == "clubscape_m1_test"
            and workload["automatic_retry"] is False,
            "The fixed journal, disposable database and no-retry gate are required.")
    require(authority["saved_world_migration_or_continuation_admitted"] is False
            and authority["milestone_accepted"] is False,
            "This runner cannot consume saved-world or milestone acceptance authority.")
    require(review["status"] == "passed"
            and review.get("fix_review", {}).get("status") == "passed"
            and authority["component_progress"]["independent_native_review_fix_still_required"] is False,
            "The independently reviewed native persisted-state repair is still required.")
    prepared = authority["prepared_source_operator"]
    execution = prepared.get("execution")
    require(isinstance(execution, dict) and execution.get("status") == "admitted"
            and type(prepared["operator_invocations_used"]) is int and prepared["operator_invocations_used"] == 0,
            "An exact final native executable admission, not preparation alone, is required.")
    require(isinstance(execution.get("candidate_head"), str)
            and re.fullmatch(r"[0-9a-f]{40}", execution["candidate_head"]) is not None
            and isinstance(execution.get("binary"), dict)
            and execution["binary"].get("path") == "target/debug/clubscape-server",
            "The final candidate and production native executable must be explicitly pinned.")
    for value in (execution.get("runner_sha256"), execution.get("review_record_sha256"),
                  execution["binary"].get("sha256")):
        require(isinstance(value, str) and re.fullmatch(r"[0-9a-f]{64}", value) is not None,
                "The admitted runner, native executable and passed review need exact SHA-256 identities.")
    require(preparation["status"] == "passed"
            and preparation["canonical_manifest_unchanged"] is True
            and preparation["account_created"] is False
            and preparation["saved_checkpoint_accessed"] is False
            and preparation["saved_world_migrated"] is False,
            "The admitted source preparation did not complete without account access.")
    worlds = {row["profile"]: row["world_id"] for row in prepared["worlds"]}
    require(len(prepared["worlds"]) == 2 and set(worlds) == {"legacy5e", "current5b"}
            and len(set(worlds.values())) == 2,
            "Exactly the two separately prepared source worlds are required.")
    for world in worlds.values():
        require(isinstance(world, str) and str(uuid.UUID(world)) == world and uuid.UUID(world).int != 0,
                "Source world identities must be canonical non-nil UUIDs.")
    profiles = preparation["profiles"]
    require(len(profiles) == 2 and {row["profile"] for row in profiles} == set(worlds),
            "Both exact source profiles must be present once.")
    pairs = {row["profile"]: row for row in authority["exact_source_pairs"]}
    require(len(authority["exact_source_pairs"]) == 2 and set(pairs) == set(worlds),
            "The admission must name exactly both distinct source transitions.")
    for row in profiles:
        profile = row["profile"]
        require(row["world_id"] == worlds[profile], "Prepared world identity changed.")
        require(pairs[profile]["from_raw_sha256"] != pairs[profile]["to_raw_sha256"],
                "A source transition must have distinct old and target artifact hashes.")
        for direction in ("from", "to"):
            source = row[direction]
            require(source["game_root"] == f".local/water-source-operator-01/{profile}/{direction}/game-root"
                    and source["identity"]["world_id"] == worlds[profile]
                    and source["identity"]["artifact_sha256"] == pairs[profile][f"{direction}_raw_sha256"]
                    and source["preparation"]["actual_source_asset_mappings"] == 5015,
                    "Prepared source profile, GameRoot, artifact or asset count changed.")
    return execution


def validate_bundle_files(root):
    total_bytes = entries = 0
    for path in root.rglob("*"):
        entries += 1
        require(entries <= 20010 and not path.is_symlink(),
                "The bounded source delivery contains excessive entries or a symlink.")
        if path.is_dir():
            continue
        require(path.is_file(), "A source delivery contains a nonregular payload.")
        total_bytes += path.stat().st_size
        require(total_bytes <= BUNDLE_MAX_BYTES, "A source delivery exceeds its 512 MiB allowance.")
    require(entries > 0, "A source delivery is empty.")


def unchanged_payload(snapshot):
    result = copy.deepcopy(snapshot)
    for field in LEASE_FIELDS:
        del result["world"][field]
    return result


def require_stopped(snapshot):
    world = snapshot["world"]
    require(world["lease_owner"] is None and world["lease_expires_at"] is None,
            "The actual server/operator did not release its world lease.")


def require_unchanged(before, after):
    require_stopped(before)
    require_stopped(after)
    require(after["world"]["lease_fence"] == before["world"]["lease_fence"] + 1,
            "A stopped operator did not acquire exactly one new fence.")
    require(unchanged_payload(before) == unchanged_payload(after),
            "A rejected or idempotent operator changed persistent state, key or journals.")


def require_migrated(before, after, profile):
    require_stopped(before)
    require_stopped(after)
    require(before["audits"] == [] and len(after["audits"]) == 1,
            "The exact first migration must create exactly one audit.")
    source, target = profile["from"], profile["to"]
    old = before["world"]
    require(old["runtime_artifact_sha256"] == source["identity"]["artifact_sha256"]
            and old["content_revision"] == source["preparation"]["source_revision"],
            "The stopped source world does not match the exact old artifact/revision.")
    audit = after["audits"][0]
    require(set(audit) == {"world_id", "from_artifact", "to_artifact", "world_revision", "migrated_at"}
            and audit["world_id"] == profile["world_id"]
            and audit["from_artifact"] == source["identity"]["artifact_sha256"]
            and audit["to_artifact"] == target["identity"]["artifact_sha256"]
            and audit["world_revision"] == old["revision"] + 1
            and isinstance(audit["migrated_at"], str) and bool(audit["migrated_at"]),
            "The migration audit does not record the exact transition.")
    expected = unchanged_payload(before)
    world = expected["world"]
    world["runtime_artifact_sha256"] = target["identity"]["artifact_sha256"]
    world["content_revision"] = target["preparation"]["source_revision"]
    world["revision"] += 1
    world["state"]["content_revision"] = world["content_revision"]
    world["state"]["revision"] += 1
    expected["audits"] = after["audits"]
    require(after["world"]["lease_fence"] == old["lease_fence"] + 1
            and unchanged_payload(after) == expected,
            "The migration changed more than the exact content pin, revision and audit.")


def require_busy_unchanged(before, after):
    old, new = before["world"], after["world"]
    require(old["lease_owner"] is not None and before["audits"] == after["audits"] == [],
            "Active-owner refusal requires an owned old world without a migration audit.")
    fields = ("world_id", "content_revision", "schema_version", "runtime_artifact_sha256",
              "runtime_random_key", "lease_owner", "lease_fence")
    require(all(old[field] == new[field] for field in fields)
            and new["tick"] >= old["tick"] and new["revision"] >= old["revision"]
            and before["counts"] == after["counts"] and before["world_ids"] == after["world_ids"],
            "The active-owner refusal changed the source pin, owner, key, journals or audit.")


def log_events(text):
    rows = [json.loads(line) for line in text.splitlines() if line.strip()]
    require(all(isinstance(row, dict) and isinstance(row.get("fields"), dict) for row in rows),
            "Native structured diagnostics were missing or malformed.")
    return rows


def require_operator_result(code, text, expected_error):
    rows = log_events(text)
    completions = [row for row in rows
                   if row["fields"].get("event") == "game_ui_migration_complete"]
    errors = [row["fields"] for row in rows if row.get("level") == "ERROR"]
    if expected_error is None:
        require(code == 0 and len(completions) == 1 and not errors,
                "The real migration CLI did not complete successfully and cleanly.")
    else:
        require(code == 1 and not completions and len(errors) == 1
                and errors[0].get("event") == "startup_failure"
                and errors[0].get("stage") == "game_ui_migration"
                and errors[0].get("error_kind") == expected_error,
                "The real migration CLI did not refuse at the required boundary.")


def reserve_journal(path, report):
    descriptor = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, 0o600)
    with os.fdopen(descriptor, "w", encoding="utf-8") as target:
        json.dump(report, target, indent=2, sort_keys=True)
        target.write("\n")
        target.flush()
        os.fsync(target.fileno())


class OperatorRun:
    def __init__(self, execution, preparation, authority_sha):
        self.execution = execution
        self.preparation = preparation
        self.output = journey.project_path(OUTPUT)
        self.journal = journey.project_path(JOURNAL)
        self.binary = journey.project_path(execution["binary"]["path"])
        self.container = "clubscape-water-operator-" + uuid.uuid4().hex[:16]
        self.container_id = None
        self.server = None
        self.env = None
        self.worlds = set()
        self.report = {
            "schema_version": 1, "status": "reserved", "phase": "reservation",
            "authority_sha256": authority_sha, "execution": execution,
            "preparation_sha256": journey.sha(journey.project_path(PREPARATION)),
            "owned_container_name": self.container,
            "server_starts": [], "operator_invocations": [], "profiles": [],
            "cleanup_errors": [], "source_accounts_or_gameplay_commands": 0,
            "runtime_key_values_recorded": False, "saved_checkpoint_accessed": False,
            "saved_world_migrated": False, "milestone_accepted": False,
        }

    def persist(self):
        journey.write_json(self.journal, self.report)
        if self.output.exists():
            journey.write_json(self.output / "report.json", self.report)

    def pin(self, profile, direction):
        require(journey.sha(self.binary) == self.execution["binary"]["sha256"],
                "The admitted native executable changed.")
        source = profile[direction]
        root = journey.project_path(source["game_root"])
        validate_bundle_files(root)
        for name in ("clubscape-game.json", "clubscape-game-assets.json"):
            journey.project_path((root / name).relative_to(journey.ROOT))
        descriptor = journey.read_json(root / "clubscape-game.json")
        journey.project_path((root / descriptor["artifact"]).relative_to(journey.ROOT))
        require(journey.game_identity(root) == source["identity"],
                "A prepared source delivery changed; the allowance is not renewed.")
        return root

    def environment(self, profile, direction):
        env = self.env.copy()
        env["CLUBSCAPE_GAME_ROOT"] = str(self.pin(profile, direction))
        return env

    def start_server(self, profile, direction):
        require(self.server is None and len(self.report["server_starts"]) < 4,
                "The four-start, one-concurrent-source-server allowance is exhausted.")
        env = self.environment(profile, direction)
        row = {"profile": profile["profile"], "direction": direction, "status": "reserved"}
        self.report["server_starts"].append(row)
        self.report["phase"] = f"{profile['profile']}/{direction}/server"
        self.persist()
        log = self.output / f"{profile['profile']}-{direction}-server.log"
        self.server = journey.OwnedServer(self.binary, env, log)
        row["origin"] = self.server.ready()
        row["status"] = "ready"
        row["log"] = log.name
        if direction == "from":
            self.worlds.add(profile["world_id"])
        self.persist()

    def stop_server(self):
        if self.server is None:
            return
        server = self.server
        row = self.report["server_starts"][-1]
        try:
            row["exit_code"] = server.stop()
            require(row["exit_code"] == 0, "The source server did not exit cleanly.")
            events = log_events(server.log_path.read_text())
            shutdowns = [event["fields"] for event in events
                         if event["fields"].get("event") == "shutdown_complete"]
            require(len(shutdowns) == 1 and shutdowns[0].get("clean") is True,
                    "Source shutdown did not report a clean completed lease/pool release.")
            row["status"] = "stopped_cleanly"
        finally:
            if server.process.poll() is not None:
                self.server = None
            self.persist()

    def operator(self, profile, name, from_hash, expected_error):
        require(len(self.report["operator_invocations"]) < 10,
                "The ten-invocation source operator allowance is exhausted.")
        env = self.environment(profile, "to")
        row = {"profile": profile["profile"], "case": name, "status": "reserved",
               "from_artifact_sha256": from_hash, "expected_error": expected_error}
        self.report["operator_invocations"].append(row)
        self.report["phase"] = f"{profile['profile']}/{name}"
        self.persist()
        log = self.output / f"{profile['profile']}-{name}.log"
        started = time.monotonic()
        with log.open("w", encoding="utf-8") as output:
            result = subprocess.run(
                [str(self.binary), "migrate-ui", "--from", from_hash],
                cwd=journey.ROOT, env=env, stdin=subprocess.DEVNULL,
                stdout=output, stderr=subprocess.STDOUT, timeout=35, check=False,
            )
        row.update(exit_code=result.returncode, elapsed_seconds=time.monotonic() - started,
                   log=log.name, log_sha256=journey.sha(log))
        self.persist()
        require_operator_result(result.returncode, log.read_text(), expected_error)
        row["status"] = "passed"
        self.persist()

    def snapshot(self, profile):
        world_id = profile["world_id"]
        require(world_id in self.worlds, "Only an already started admitted source world may be read.")
        counts = ", ".join(f"'{name}', (SELECT count(*) FROM {name})" for name in COUNTS)
        query = f"""
SELECT jsonb_build_object(
    'world', (SELECT to_jsonb(w) FROM game_worlds w WHERE world_id = '{world_id}'::uuid),
    'world_ids', (SELECT jsonb_agg(world_id ORDER BY world_id) FROM game_worlds),
    'counts', jsonb_build_object({counts}),
    'audits', COALESCE((SELECT jsonb_agg(to_jsonb(a) ORDER BY to_artifact)
        FROM game_content_migrations a WHERE world_id = '{world_id}'::uuid), '[]'::jsonb));
"""
        result = subprocess.run(
            ["docker", "exec", "--interactive", "--env", "PGOPTIONS=-c statement_timeout=5000",
             self.container_id, "psql", "--no-psqlrc", "--tuples-only", "--no-align",
             "--set", "ON_ERROR_STOP=1", "--username=clubscape", "--dbname=clubscape_m1_test"],
            input=query, capture_output=True, text=True, timeout=10, check=False,
        )
        require(result.returncode == 0 and len(result.stdout) <= journey.MAX_SOURCE_BYTES,
                "The owned database snapshot failed or exceeded its bound; private output withheld.")
        snapshot = json.loads(result.stdout)
        require(snapshot["counts"] == dict.fromkeys(COUNTS, 0)
                and set(snapshot["world_ids"]) == self.worlds
                and len(snapshot["world_ids"]) == len(self.worlds),
                "The disposable database contains an unexpected world, account or journal.")
        world = snapshot["world"]
        require(isinstance(world, dict) and world["world_id"] == world_id
                and world["state"]["characters"] == {}
                and isinstance(world["runtime_random_key"], str)
                and re.fullmatch(r"\\x[0-9a-f]{64}", world["runtime_random_key"]) is not None,
                "The admitted world is not an empty-character source world with a native key.")
        for field in ("content_revision", "schema_version", "revision", "tick"):
            require(world[field] == world["state"][field], "World columns and state disagree.")
        return snapshot

    def profile(self, profile):
        self.start_server(profile, "from")
        before = self.snapshot(profile)
        self.operator(profile, "active-owner", profile["from"]["identity"]["artifact_sha256"],
                      "exclusive_world_ownership")
        require_busy_unchanged(before, self.snapshot(profile))
        self.stop_server()
        before = self.snapshot(profile)
        require_stopped(before)
        self.operator(profile, "wrong-pin", "0" * 64, "state_migration")
        after = self.snapshot(profile)
        require_unchanged(before, after)
        before = after
        old_hash = profile["from"]["identity"]["artifact_sha256"]
        self.operator(profile, "migrate", old_hash, None)
        after = self.snapshot(profile)
        require_migrated(before, after, profile)
        for name, from_hash, error in (
            ("idempotent-retry", old_hash, None),
            ("wrong-origin-retry", "0" * 64, "state_migration"),
        ):
            before = after
            self.operator(profile, name, from_hash, error)
            after = self.snapshot(profile)
            require_unchanged(before, after)
        before = after
        self.start_server(profile, "to")
        self.stop_server()
        after = self.snapshot(profile)
        require_stopped(after)
        old, new = before["world"], after["world"]
        require(all(old[field] == new[field] for field in (
            "world_id", "schema_version", "content_revision", "runtime_artifact_sha256", "runtime_random_key",
        )) and before["audits"] == after["audits"]
            and new["lease_fence"] == old["lease_fence"] + 1
            and new["tick"] >= old["tick"] and new["revision"] >= old["revision"],
            "Normal target restart changed a pin/key/audit or did not acquire a replacement fence.")
        self.report["profiles"].append({
            "profile": profile["profile"], "world_id": profile["world_id"], "status": "passed",
            "from_artifact_sha256": old_hash,
            "to_artifact_sha256": profile["to"]["identity"]["artifact_sha256"],
            "migration_audits": len(after["audits"]), "runtime_key_unchanged": True,
            "state_conserved_except_exact_content_transition": True,
            "accounts_and_command_journals_empty": True, "target_normal_startup_ready": True,
        })
        self.persist()

    def database(self, secret):
        result = journey.bounded([
            "docker", "run", "--detach", "--rm", "--pull=never", "--name", self.container,
            "--label", "clubscape.scope=m1-water-source-operator",
            "--label", f"clubscape.owner={self.container}",
            "--cpus=2", "--memory=512m", "--pids-limit=256", "--read-only",
            "--tmpfs", "/var/lib/postgresql/data:rw,nosuid,nodev,size=384m",
            "--tmpfs", "/var/run/postgresql:rw,nosuid,nodev,size=16m",
            "--tmpfs", "/tmp:rw,nosuid,nodev,size=16m",
            "--mount", f"type=bind,src={secret},dst=/run/secrets/postgres-password,readonly",
            "--env", "POSTGRES_USER=clubscape", "--env", "POSTGRES_DB=clubscape_m1_test",
            "--env", "POSTGRES_PASSWORD_FILE=/run/secrets/postgres-password",
            "--publish", "127.0.0.1::5432", journey.POSTGRES_IMAGE,
        ], timeout=120)
        self.container_id = result.stdout.strip()
        require(re.fullmatch(r"[0-9a-f]{64}", self.container_id) is not None,
                "Docker did not return the exact owned container ID.")
        deadline = time.monotonic() + 60
        while time.monotonic() < deadline:
            result = journey.bounded([
                "docker", "exec", self.container_id, "pg_isready", "-h", "127.0.0.1",
                "-U", "clubscape", "-d", "clubscape_m1_test",
            ], timeout=min(10, max(0.1, deadline - time.monotonic())), allow_failure=True)
            if result.returncode == 0:
                break
            time.sleep(0.25)
        else:
            raise journey.JourneyError("The owned database exceeded its readiness deadline.")
        binding = journey.bounded(["docker", "port", self.container_id, "5432/tcp"], timeout=10).stdout.strip()
        match = re.fullmatch(r"127\.0\.0\.1:([0-9]+)", binding)
        require(match is not None, "The owned database did not bind a random literal loopback port.")
        self.report["owned_container_id"] = self.container_id
        self.report["database_image"] = journey.POSTGRES_IMAGE
        self.report["database_ready_on_owned_loopback"] = True
        return int(match[1])

    def cleanup_database(self):
        found = journey.bounded([
            "docker", "ps", "--all", "--no-trunc", "--filter", f"name=^/{self.container}$",
            "--format", "{{.ID}}",
        ], timeout=10).stdout.strip()
        if not found:
            self.report["owned_database_removed"] = True
            return
        require(re.fullmatch(r"[0-9a-f]{64}", found) is not None
                and (self.container_id is None or found == self.container_id),
                "The exact owned container identity is ambiguous.")
        labels = json.loads(journey.bounded([
            "docker", "inspect", "--format", "{{json .Config.Labels}}", found,
        ], timeout=10).stdout)
        require(labels.get("clubscape.scope") == "m1-water-source-operator"
                and labels.get("clubscape.owner") == self.container,
                "Refusing cleanup of a database not owned by this invocation.")
        journey.bounded(["docker", "rm", "--force", found], timeout=30)
        remaining = journey.bounded([
            "docker", "ps", "--all", "--filter", f"id={found}", "--format", "{{.ID}}",
        ], timeout=10).stdout.strip()
        require(not remaining, "The exact owned database remained after cleanup.")
        self.report["owned_database_removed"] = True

    def execute(self):
        sys.path.insert(0, str(journey.ROOT / "tools"))
        import dev

        require(journey.POSTGRES_IMAGE == dev.POSTGRES_IMAGE, "Pinned PostgreSQL image disagreement.")
        require(not self.output.exists() and not self.journal.exists(),
                "The fixed output or journal already exists; there is no automatic retry.")
        journey.private_directory(Path(JOURNAL).parent)
        reserve_journal(self.journal, self.report)
        secret = None
        succeeded = False
        try:
            journey.private_directory(OUTPUT)
            with tempfile.TemporaryDirectory(prefix="credentials-", dir=self.output) as temporary:
                secret = Path(temporary) / "postgres-password"
                password = dev.write_secret(secret)
                try:
                    self.report["phase"] = "database_start"
                    self.persist()
                    port = self.database(secret)
                    self.env = {key: value for key, value in os.environ.items()
                                if not key.startswith("CLUBSCAPE_") and key != "DATABASE_URL"}
                    self.env.update(
                        DATABASE_URL=dev.database_url(password, port, "clubscape_m1_test"),
                        CLUBSCAPE_BIND="127.0.0.1:0",
                        CLUBSCAPE_BUILD_REVISION=self.execution["candidate_head"],
                    )
                    for profile in self.preparation["profiles"]:
                        self.profile(profile)
                    require(len(self.report["server_starts"]) == 4
                            and len(self.report["operator_invocations"]) == 10
                            and len(self.report["profiles"]) == 2,
                            "The exact four starts and ten operator cases did not complete.")
                    succeeded = True
                except (journey.JourneyError, OSError, ValueError, KeyError, subprocess.TimeoutExpired) as error:
                    self.report["failure"] = {"phase": self.report["phase"],
                                              "reason": journey.redact(str(error), self.env)}
                    raise
                finally:
                    for cleanup in (self.stop_server, self.cleanup_database):
                        try:
                            cleanup()
                        except (journey.JourneyError, OSError, ValueError, subprocess.TimeoutExpired) as error:
                            self.report["cleanup_errors"].append(str(error))
        except (journey.JourneyError, OSError, ValueError, KeyError, subprocess.TimeoutExpired) as error:
            self.report.setdefault("failure", {"phase": self.report["phase"],
                                               "reason": journey.redact(str(error), self.env)})
            raise
        finally:
            self.report["temporary_credentials_removed"] = secret is None or not secret.exists()
            clean = not self.report["cleanup_errors"] and self.report["temporary_credentials_removed"]
            self.report["status"] = "passed" if succeeded and clean else "failed"
            if self.report["status"] == "passed":
                self.report["phase"] = "complete"
            self.persist()
        require(self.report["status"] == "passed", "The operator preflight did not clean up successfully.")
        print(json.dumps({key: self.report[key] for key in (
            "status", "profiles", "owned_database_removed", "temporary_credentials_removed",
            "saved_world_migrated", "milestone_accepted",
        )}))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--admission-sha256", required=True)
    args = parser.parse_args()
    require(re.fullmatch(r"[0-9a-f]{64}", args.admission_sha256) is not None,
            "The exact current main admission SHA-256 is required.")
    common = Path(journey.bounded([
        "git", "rev-parse", "--path-format=absolute", "--git-common-dir",
    ], timeout=10).stdout.strip())
    repo = common.parent.resolve(strict=True)
    require(journey.ROOT == repo / WORKTREE, "Run only in the admitted water integration worktree.")
    authority_path = journey.project_path(AUTHORITY, root=repo)
    require(journey.sha(authority_path) == args.admission_sha256, "The main admission changed.")
    authority = journey.read_json(authority_path)
    review_path = journey.project_path(REVIEW, root=repo)
    preparation_path = journey.project_path(PREPARATION)
    preparation = journey.read_json(preparation_path)
    execution = validate_admission(authority, journey.read_json(review_path), preparation)
    require(journey.sha(preparation_path) == authority["prepared_source_operator"]["preparation_sha256"]
            and journey.sha(review_path) == execution["review_record_sha256"]
            and journey.sha(Path(__file__)) == execution["runner_sha256"]
            and journey.sha(Path(journey.__file__)) == authority["prepared_source_operator"]["source_packager_sha256"]
            and journey.sha(journey.project_path("content/m1/manifest.json"))
            == preparation["canonical_manifest_sha256"],
            "Preparation, passed review or operator program identity changed.")
    require(journey.bounded(["git", "rev-parse", "HEAD"], timeout=10).stdout.strip()
            == execution["candidate_head"]
            and not journey.bounded(["git", "status", "--porcelain", "--untracked-files=no"],
                                    timeout=10).stdout.strip(),
            "The final candidate commit must be pinned and its tracked worktree clean.")
    runner = OperatorRun(execution, preparation, args.admission_sha256)
    for profile in preparation["profiles"]:
        for direction in ("from", "to"):
            runner.pin(profile, direction)
    signal.signal(signal.SIGTERM, journey.interrupted)
    signal.signal(signal.SIGINT, journey.interrupted)
    runner.execute()


if __name__ == "__main__":
    try:
        main()
    except (journey.JourneyError, OSError, ValueError, KeyError, subprocess.TimeoutExpired) as error:
        print(f"Water operator preflight failed: {error}", file=sys.stderr)
        sys.exit(1)

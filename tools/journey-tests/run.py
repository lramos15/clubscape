#!/usr/bin/env python3
"""Own a bounded loopback PostgreSQL/server run; never seed gameplay or accept M1."""

from __future__ import annotations

import argparse
import gzip
import hashlib
import json
import os
from pathlib import Path
import re
import secrets
import shutil
import signal
import stat
import subprocess
import sys
import time
import urllib.error
import urllib.parse
import urllib.request
import uuid

from private_checkpoint import CheckpointError, preserve_checkpoint


ROOT = Path(__file__).resolve().parents[2]
POSTGRES_IMAGE = (
    "postgres:16-alpine@sha256:"
    "cf78e76683b9ca8c5733cbbdce6c9262b45b6767934dd0a95e671f9a0fc20685"
)
SEGMENTS = (
    "registration_login", "source_initial_character",
    "full_tutorial_learning_the_ropes", "onboarding_recovery", "lumbridge_copper",
    "inventory_equipment_bank_shop", "goblin_combat",
    "source_death_office_grave_recovery",
    "cooks_legitimate_acquisition_partial_delivery", "cooks_reward_and_range",
    "after_quest_recovery",
)
MAX_SOURCE_BYTES = 64 * 1024 * 1024


class JourneyError(RuntimeError):
    pass


def require(condition, message):
    if not condition:
        raise JourneyError(message)


def sha(path):
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for block in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def project_path(value, *, root=ROOT):
    path = Path(value)
    require(
        not path.is_absolute() and bool(path.parts)
        and all(part not in {".", ".."} for part in path.parts),
        "All writes must use relative project paths without traversal.",
    )
    current = root
    for part in path.parts:
        current /= part
        require(not current.is_symlink(), "Refusing a symlinked project output.")
    require(current.resolve().is_relative_to(root.resolve()), "Output escaped the project.")
    return current


def private_directory(value):
    path = project_path(value)
    path.mkdir(parents=True, exist_ok=True, mode=0o700)
    metadata = path.stat()
    require(
        metadata.st_uid == os.getuid() and not stat.S_IMODE(metadata.st_mode) & 0o077,
        f"Run directory must be owned by this user with mode 0700: {value}",
    )
    return path


def write_json(path, value, *, compact=False):
    require(path.is_relative_to(ROOT), "Evidence must remain in this project.")
    relative = path.relative_to(ROOT)
    project_path(relative)
    path.parent.mkdir(parents=True, exist_ok=True, mode=0o700)
    pending = path.with_suffix(".pending.json")
    project_path(pending.relative_to(ROOT))
    descriptor = os.open(pending, os.O_WRONLY | os.O_CREAT | os.O_TRUNC, 0o600)
    with os.fdopen(descriptor, "w", encoding="utf-8") as target:
        json.dump(value, target, indent=None if compact else 2, sort_keys=True,
                  separators=(",", ":") if compact else None)
        target.write("\n")
        target.flush()
        os.fsync(target.fileno())
    os.replace(pending, path)


def read_json(path):
    require(path.stat().st_size <= MAX_SOURCE_BYTES, f"JSON exceeds source input bound: {path}")
    if path.suffix == ".gz":
        with gzip.open(path, "rb") as source:
            data = source.read(MAX_SOURCE_BYTES + 1)
    else:
        data = path.read_bytes()
    require(len(data) <= MAX_SOURCE_BYTES, "Expanded source exceeds bound.")
    return json.loads(data)


def origin(value):
    parsed = urllib.parse.urlsplit(value)
    require(
        parsed.scheme == "http" and parsed.hostname == "127.0.0.1"
        and parsed.username is None and parsed.password is None
        and parsed.path in {"", "/"} and not parsed.query and not parsed.fragment
        and parsed.port is not None,
        "Owned service must announce a literal loopback HTTP origin.",
    )
    return value.rstrip("/")


def bounded(arguments, *, env=None, timeout=600, allow_failure=False):
    result = subprocess.run(
        [str(argument) for argument in arguments], cwd=ROOT, env=env,
        stdin=subprocess.DEVNULL, capture_output=True, text=True,
        timeout=timeout, check=False,
    )
    if result.returncode and not allow_failure:
        text = redact(result.stdout + result.stderr, env)
        raise JourneyError(f"{arguments[0]} failed ({result.returncode}): {text[-9000:]}")
    return result


def redact(text, env=None):
    for name in ("DATABASE_URL", "CLUBSCAPE_TEST_DATABASE_URL"):
        value = (env or {}).get(name)
        if value:
            text = text.replace(value, "<redacted isolated database URL>")
    return text


def tail(path, maximum=9000):
    if not path.exists():
        return ""
    with path.open("rb") as source:
        source.seek(max(0, path.stat().st_size - maximum))
        return source.read(maximum).decode("utf-8", errors="replace")


def full_journey_passed(report):
    return (
        report.get("scenario") == "m1_fresh_account"
        and report.get("status") == "passed"
        and report.get("full_journey_passed") is True
        and report.get("milestone_accepted") is False
        and len(report.get("tutorial_edges_passed", [])) == 70
        and all(
            report.get("segments", {}).get(segment, {}).get("status") == "passed"
            for segment in SEGMENTS
        )
    )

def payload_locations(index):
    require(type(index) is int and 0 <= index < 20000, "Invalid bounded payload index.")
    return f"/assets/{index:x}", f"assets/{index:x}.bin"

def record_server_exit(report, code):
    report["owned_server_exit_code"] = code
    report["server_exit_clean"] = code == 0
    report["owned_server_reaped"] = True
    if code != 0:
        report["status"] = "blocked"
        report["full_journey_passed"] = False
        report.setdefault("first_failure", {
            "phase": "server_shutdown",
            "reason": f"The owned real server exited unsuccessfully ({code}); resource cleanup is not clean-world success.",
        })


def preserve_blocked_checkpoint(directory, report, server):
    observation = (report.get("current_phase") == "real_m1_dying_observation"
                   and report.get("observation_only") is True)
    if (not observation and (report.get("current_phase") != "real_m1_fresh_account_scenario"
                            or report.get("status") == "passed")):
        return {
            "status": "not_attempted", "snapshot_available": False,
            "reason": "not_a_blocked_source_scenario", "resume_authorized": False,
        }
    try:
        return preserve_checkpoint(ROOT, directory, report, server)
    except (CheckpointError, OSError, ValueError, KeyError, TypeError,
            subprocess.TimeoutExpired, JourneyError) as error:
        return {
            "status": "failed", "snapshot_available": False,
            "recoverable_checkpoint": False, "resume_authorized": False,
            "phase": error.phase if isinstance(error, CheckpointError) else "checkpoint_setup",
            "reason": error.code if isinstance(error, CheckpointError)
            else f"os_error_{error.errno}" if isinstance(error, OSError)
            else "checkpoint_capture_incomplete",
        }


def preserve_and_cleanup(directory, name, report, server, cleanup_errors, report_path):
    try:
        report["private_checkpoint"] = {
            "status": "failed", "snapshot_available": False,
            "recoverable_checkpoint": False, "resume_authorized": False,
            "reason": "capture_did_not_complete",
        }
        report["private_checkpoint"] = preserve_blocked_checkpoint(directory, report, server)
    finally:
        try:
            cleanup_container(name, report)
        except (JourneyError, OSError, ValueError, subprocess.TimeoutExpired) as error:
            cleanup_errors.append(str(error))
        try:
            require(directory.parent == ROOT / ".local/journey-runs", "Refusing unowned directory cleanup.")
            shutil.rmtree(directory)
            report["owned_credentials_and_game_root_removed"] = True
        except (JourneyError, OSError) as error:
            cleanup_errors.append(str(error))
        report["cleanup_errors"] = cleanup_errors
        report["cleanup_passed"] = not cleanup_errors
        if cleanup_errors:
            report["status"] = "blocked"
            report["full_journey_passed"] = False
        report["milestone_accepted"] = False
        write_json(report_path, report)


class OwnedServer:
    def __init__(self, binary, env, log_path):
        self.binary = binary
        self.env = env
        self.log_path = log_path
        self.log = log_path.open("w", encoding="utf-8")
        self.process = subprocess.Popen(
            [str(binary)], cwd=ROOT, env=env, stdin=subprocess.DEVNULL,
            stdout=self.log, stderr=subprocess.STDOUT,
        )
        self.origin = None

    def ready(self):
        deadline = time.monotonic() + 35
        opener = urllib.request.build_opener(urllib.request.ProxyHandler({}))
        with self.log_path.open(encoding="utf-8") as log:
            while time.monotonic() < deadline:
                if self.process.poll() is not None:
                    raise JourneyError(
                        "Real server exited before readiness "
                        f"(exit {self.process.returncode}):\n{redact(tail(self.log_path), self.env)}"
                    )
                line = log.readline()
                if not line:
                    time.sleep(0.05)
                    continue
                try:
                    event = json.loads(line).get("fields", {})
                except json.JSONDecodeError:
                    continue
                if event.get("event") == "listening":
                    self.origin = origin("http://" + event.get("address", ""))
                    with opener.open(self.origin + "/healthz", timeout=5) as response:
                        require(response.status == 200, "Real database readiness endpoint failed.")
                        response.read(65536)
                    require(self.process.poll() is None, "Owned process died after health readiness.")
                    return self.origin
        raise JourneyError("Real server did not become loopback/database-ready in 35 seconds.")

    def stop(self, *, crash=False):
        process = self.process
        forced = False
        if process.poll() is None:
            if crash:
                process.kill()
            else:
                process.terminate()
            try:
                process.wait(timeout=20)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait(timeout=10)
                forced = True
        self.log.close()
        require(not forced, "Owned server required unexpected forced termination.")
        return process.returncode


def verified_collection_locations(references):
    """Use the canonical publication validator, then retain exact original shard bytes."""
    prior_path = list(sys.path)
    prior_bytecode = sys.dont_write_bytecode
    try:
        sys.dont_write_bytecode = True
        sys.path.insert(0, str(ROOT / "tools/cache-import"))
        from content_closure import load_published_inputs, publication_chain
        publication = project_path(references["publications"][-1])
        bundle, collections = load_published_inputs(publication)
        require(
            {record["asset_id"] for record in bundle["records"]}.issuperset(
                asset["id"] for asset in references["assets"]
            ),
            "Product references are absent from the validated original catalog.",
        )
        shards = {
            path: path.name.split(".")[0]
            for path in (ROOT / "assets/source/osrs/cache2695/collections").glob("*.json.gz")
        }
        for _, manifest in publication_chain(publication):
            for kind, entry in manifest.get("collection_extensions", {}).items():
                shards[project_path(entry["path"])] = kind
        locations = {}
        for path, kind in shards.items():
            for asset_id, record in read_json(path).items():
                require(record == collections[kind].get(asset_id), "Original collection payload differs from validated source.")
                require(asset_id not in locations, "Duplicate original asset in collection shards.")
                locations[asset_id] = path
        return locations
    except JourneyError:
        raise
    except Exception as error:
        raise JourneyError(f"Canonical original-publication validation failed: {error}") from error
    finally:
        sys.path[:] = prior_path
        sys.dont_write_bytecode = prior_bytecode


def publish_product_root(directory, report, inspector, env):
    """Expose genuine original bytes for every asset required by the real strict compiler."""
    game_root = directory / "game-root"
    game_root.mkdir(mode=0o700)
    content_manifest_path = ROOT / "content/m1/manifest.json"
    content_manifest = read_json(content_manifest_path)
    compiled = content_manifest["compiled_artifact"]
    archive_path = project_path(compiled["path"])
    require(sha(archive_path) == compiled["sha256"], "Product artifact archive checksum differs from manifest.")
    with gzip.open(archive_path, "rb") as source:
        artifact = source.read(MAX_SOURCE_BYTES + 1)
    require(len(artifact) <= MAX_SOURCE_BYTES, "Product artifact exceeds bounded GameRoot input.")
    artifact_sha = hashlib.sha256(artifact).hexdigest()
    require(artifact_sha == compiled["uncompressed_sha256"], "Compiled product checksum differs from manifest.")
    (game_root / "world.csc").write_bytes(artifact)
    inspected = json.loads(bounded([
        inspector, "inspect-artifact", game_root / "world.csc",
    ], env=env, timeout=120).stdout)
    require(inspected["artifact_sha256"] == artifact_sha, "Strict compiler inspected a different artifact.")
    required_assets = set(inspected["referenced_assets"])
    require(0 < len(required_assets) <= 20000, "Unbounded/empty actual compiler asset-reference set.")
    references = read_json(ROOT / "content/m1/asset-references.json")
    original_collections = verified_collection_locations(references)
    catalog = {entry["id"]: entry for entry in references["assets"]}
    published = {}
    for name in references["publications"]:
        for entry in read_json(project_path(name))["published_files"]:
            published.setdefault(entry["sha256"], []).append(entry)
    files = []
    assets = {}
    copied = {}
    missing = []
    collection_deliveries = 0
    payload_hashes = {}
    total_bytes = len(artifact)
    for asset_id in sorted(required_assets):
        asset = catalog.get(asset_id)
        require(asset is not None, f"Compiler asset missing from original catalog: {asset_id}")
        matched = None
        if asset_id in original_collections:
            path = original_collections[asset_id]
            if path not in payload_hashes:
                payload_hashes[path] = sha(path)
            matched = (path, payload_hashes[path])
            collection_deliveries += 1
        else:
            for output in asset["outputs"]:
                for entry in published.get(output["sha256"], []):
                    path = project_path(entry["path"])
                    if not path.is_file():
                        continue
                    require(sha(path) == output["sha256"], f"Published source asset changed: {entry['path']}")
                    matched = (path, output["sha256"])
                    break
                if matched:
                    break
        if matched is None:
            missing.append(asset["id"])
            continue
        source, digest = matched
        if digest not in copied:
            url, relative = payload_locations(len(copied))
            destination = game_root / relative
            destination.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(source, destination)
            total_bytes += destination.stat().st_size
            require(total_bytes <= 512 * 1024 * 1024, "Product GameRoot exceeds server static-byte budget.")
            files.append({
                "url": url, "path": relative, "sha256": digest,
                "content_type": "application/octet-stream",
            })
            copied[digest] = url
        assets[asset_id] = copied[digest]
    require(not missing, f"Required genuine source outputs are absent: {missing[:16]} ({len(missing)} total)")
    relative = "content/manifest.json"
    (game_root / "content").mkdir()
    shutil.copyfile(content_manifest_path, game_root / relative)
    files.append({
        "url": "/" + relative, "path": relative, "sha256": sha(content_manifest_path),
        "content_type": "application/json",
    })
    descriptor = {
        "schema_version": 1, "world_id": str(uuid.uuid4()), "artifact": "world.csc",
        "sha256": artifact_sha, "content_manifest_path": "/content/manifest.json",
        "assets": assets,
        "readiness_profile": {
            "id": "ordinary_normal_f2p",
            "excluded_items": ["item.ensouled_goblin_head", "item.milk.bottomless_bucket"],
        },
    }
    write_json(game_root / "clubscape-game.json", descriptor, compact=True)
    write_json(game_root / "clubscape-game-assets.json", {"schema_version": 1, "files": files})
    report["product_root_preparation"] = {
        "artifact": compiled, "source_revision": content_manifest["revision"],
        "actual_source_asset_mappings": len(assets),
        "actual_compiler_referenced_assets": len(required_assets),
        "all_compiler_referenced_assets_mapped": len(assets) == len(required_assets),
        "original_collection_deliveries": collection_deliveries,
        "original_payload_files": len(copied),
        "source_publication_validation": "passed",
        "collection_delivery": "Original hash-validated shard bytes, keyed by the unchanged original asset ID; no placeholder or regenerated model/definition.",
        "readiness_profile": descriptor["readiness_profile"],
        "compiler_unresolved_binding_paths": inspected["unresolved_bindings"],
        "descriptor_bytes": (game_root / "clubscape-game.json").stat().st_size,
        "known_descriptor_limit_at_bcc0de9_bytes": 256 * 1024,
        "unmapped_original_asset_ids_count": len(missing),
        "unmapped_original_asset_ids_first_24": missing[:24],
        "unmapped_are_not_placeholders": True,
        "world_id": descriptor["world_id"],
        "descriptor_sha256": sha(game_root / "clubscape-game.json"),
        "game_assets_manifest_sha256": sha(game_root / "clubscape-game-assets.json"),
        "policy": "Real server still enforces descriptor limits, strict artifact/source-profile readiness, actual assets and engine initialization. Original asset bytes are not UI/gameplay acceptance.",
    }
    return game_root


def game_identity(game_root):
    descriptor_path = game_root / "clubscape-game.json"
    descriptor = read_json(descriptor_path)
    artifact = Path(descriptor["artifact"])
    require(not artifact.is_absolute() and ".." not in artifact.parts, "Unsafe artifact selector.")
    require(not (game_root / artifact).is_symlink(), "GameRoot artifact cannot be a symlink.")
    return {
        "descriptor_sha256": sha(descriptor_path),
        "artifact_sha256": sha(game_root / artifact),
        "assets_manifest_sha256": sha(game_root / "clubscape-game-assets.json"),
        "world_id": descriptor["world_id"],
    }


def start_database(directory, name, report):
    password = secrets.token_urlsafe(32)
    secret = directory / "postgres-password"
    descriptor = os.open(secret, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    with os.fdopen(descriptor, "w", encoding="ascii") as file:
        file.write(password + "\n")
    container = bounded([
        "docker", "run", "--detach", "--rm", "--name", name,
        "--label", "clubscape.scope=m1-headless-journey", "--label", f"clubscape.owner={name}",
        "--cpus=2", "--memory=768m", "--pids-limit=256", "--read-only",
        "--tmpfs", "/var/lib/postgresql/data:rw,nosuid,nodev,size=512m",
        "--tmpfs", "/var/run/postgresql:rw,nosuid,nodev,size=16m",
        "--mount", f"type=bind,src={secret},dst=/run/secrets/postgres-password,readonly",
        "--env", "POSTGRES_USER=clubscape", "--env", "POSTGRES_DB=clubscape_journey",
        "--env", "POSTGRES_PASSWORD_FILE=/run/secrets/postgres-password",
        "--publish", "127.0.0.1::5432", POSTGRES_IMAGE,
    ], timeout=120).stdout.strip()
    require(re.fullmatch(r"[0-9a-f]{64}", container), "Docker did not return an owned container identity.")
    report["owned_container_id"] = container
    deadline = time.monotonic() + 75
    while time.monotonic() < deadline:
        ready = bounded([
            "docker", "exec", container, "pg_isready", "-h", "127.0.0.1",
            "-U", "clubscape", "-d", "clubscape_journey",
        ], timeout=10, allow_failure=True)
        if ready.returncode == 0:
            break
        time.sleep(0.5)
    else:
        raise JourneyError("Owned PostgreSQL did not become ready in 75 seconds.")
    binding = bounded(["docker", "port", container, "5432/tcp"], timeout=10).stdout.strip()
    match = re.fullmatch(r"127\.0\.0\.1:(\d+)", binding)
    require(match is not None, "Owned database did not bind one random literal loopback port.")
    return f"postgresql://clubscape:{urllib.parse.quote(password, safe='')}@127.0.0.1:{match[1]}/clubscape_journey"


def cleanup_container(name, report):
    inspection = bounded(["docker", "inspect", "--format", "{{json .Config.Labels}}", name],
                         timeout=15, allow_failure=True)
    if inspection.returncode:
        return
    labels = json.loads(inspection.stdout)
    require(
        labels.get("clubscape.scope") == "m1-headless-journey"
        and labels.get("clubscape.owner") == name,
        "Refusing to stop a container not owned by this exact invocation.",
    )
    bounded(["docker", "rm", "--force", name], timeout=30)
    check = bounded(["docker", "inspect", name], timeout=10, allow_failure=True)
    require(check.returncode != 0, "Owned PostgreSQL container still exists after cleanup.")
    report["owned_database_removed"] = True


def run(args):
    run_id = uuid.uuid4().hex[:16]
    directory = private_directory(f".local/journey-runs/{run_id}")
    control = directory / "control"
    control.mkdir(mode=0o700)
    report_path = project_path(args.report)
    report_path.parent.mkdir(parents=True, exist_ok=True, mode=0o700)
    evidence_directory = report_path.parent / f"journey-{run_id}"
    evidence_directory.mkdir(mode=0o700)
    name = f"clubscape-journey-{run_id}"
    report = {
        "schema_version": 1, "kind": "controlled_real_protocol_journey",
        "run_id": run_id, "recorded_at_unix_ms": int(time.time() * 1000),
        "status": "blocked", "full_journey_passed": False, "milestone_accepted": False,
        "browser_signup_verified": False, "presentation_verified": False,
        "audio_verified": False, "performance_verified": False, "runelite_verified": False,
        "source_state_seeded": False, "gameplay_sql_used": False,
        "database_image": POSTGRES_IMAGE,
        "server_entrypoint": args.server_entrypoint,
        "resource_limits": {"cpus": 2, "memory_mib": 768, "pids": 256},
        "owned_container_name": name, "commands": [], "restarts": [],
        "unchecked_segments": list(SEGMENTS),
        "private_checkpoint": {
            "status": "not_attempted", "snapshot_available": False, "resume_authorized": False,
        },
    }
    server = None
    simulator = None
    env = os.environ.copy()
    try:
        for key in ("CLUBSCAPE_GAME_ROOT", "CLUBSCAPE_WEB_ROOT", "CLUBSCAPE_TEST_DATABASE_URL"):
            env.pop(key, None)
        build_work = private_directory(".local/journey-build-work")
        env["TMPDIR"] = str(build_work)
        env["CARGO_TARGET_DIR"] = str(project_path(args.target_dir))
        revision = bounded(["git", "rev-parse", "HEAD"]).stdout.strip()
        report["revision"] = revision
        report["revision_provenance"] = "Workspace HEAD label; exact executable SHA-256 is recorded separately."
        report["build_performed_by_orchestrator"] = not args.skip_build
        report["workspace_dirty"] = bool(bounded(["git", "status", "--porcelain"]).stdout.strip())
        report["host"] = {"machine": os.uname().machine, "system": os.uname().sysname}
        build = [
            "cargo", "build", "--quiet", "-p", "clubscape-server", "-p", "clubscape-sim",
            "--features", "clubscape-sim/journey-server",
        ]
        if not args.skip_build:
            bounded(build, env=env, timeout=900)
            report["commands"].append({"command": " ".join(build), "result": "passed"})
        report["cargo_lock_sha256_used_for_build"] = sha(ROOT / "Cargo.lock")
        account_binary = Path(env["CARGO_TARGET_DIR"]) / "debug/clubscape-server"
        binary = Path(env["CARGO_TARGET_DIR"]) / (
            "debug/clubscape-journey-server" if args.server_entrypoint == "public-api"
            else "debug/clubscape-server"
        )
        client = Path(env["CARGO_TARGET_DIR"]) / "debug/clubscape-sim"
        inspector = Path(env["CARGO_TARGET_DIR"]) / "debug/clubscape-journey-server"
        require(binary.is_file() and client.is_file(), "Chosen validation command requires built server/simulator binaries.")
        report["binary_sha256"] = {
            "server": sha(binary), "simulator": sha(client), "account_server": sha(account_binary),
        }
        env["DATABASE_URL"] = start_database(directory, name, report)
        env["CLUBSCAPE_BIND"] = "127.0.0.1:0"
        env["CLUBSCAPE_BUILD_REVISION"] = revision
        server = OwnedServer(account_binary, env.copy(), evidence_directory / "accounts-server.jsonl")
        address = server.ready()
        account = bounded([client, "account-lifecycle", "--url", address], env=env, timeout=90)
        account_report = json.loads(account.stdout)
        require(account_report.get("result") == "passed" and account_report.get("milestone_accepted") is False,
                "Independent real account lifecycle failed.")
        report["account_lifecycle"] = account_report
        report["commands"].append({
            "command": "clubscape-sim account-lifecycle --url <owned-loopback-origin>",
            "result": "passed", "checks": len(account_report["checks"]),
        })
        unavailable = evidence_directory / "unavailable-scenario.json"
        negative = bounded([
            client, "scenario", "m1_fresh_account", "--url", address,
            "--report", unavailable.relative_to(ROOT), "--expected-server-build", revision,
            "--max-seconds", "30",
        ], env=env, timeout=60, allow_failure=True)
        negative_report = read_json(unavailable)
        report["source_identity"] = negative_report.get("identity")
        report["negative_readiness_report"] = str(unavailable.relative_to(ROOT))
        require(
            negative.returncode != 0 and negative_report.get("full_journey_passed") is False
            and "not gameplay-ready" in negative_report.get("first_failure", {}).get("reason", ""),
            "The real account-only negative scenario did not fail specifically at gameplay readiness.",
        )
        report["commands"].append({
            "command": "clubscape-sim scenario m1_fresh_account --url <real-account-only-server>",
            "result": "passed_negative_readiness_check",
            "gameplay_verified": False, "scenario_exit_code": negative.returncode,
        })
        require(server.stop() == 0, "Account-only server did not shut down cleanly.")
        server = None

        if args.game_root:
            game_root = project_path(args.game_root)
            require(game_root.is_dir(), "Supplied product GameRoot is absent.")
        else:
            report["current_phase"] = "genuine_product_root_assembly"
            game_root = publish_product_root(directory, report, inspector, env)
        report["game_root_identity"] = game_identity(game_root)
        expected_artifact = read_json(ROOT / "content/m1/manifest.json")["compiled_artifact"]["uncompressed_sha256"]
        require(report["game_root_identity"]["artifact_sha256"] == expected_artifact,
                "Supplied GameRoot artifact differs from the simulator's source/content manifest.")
        env["CLUBSCAPE_GAME_ROOT"] = str(game_root)
        report["current_phase"] = "strict_product_server_startup"
        write_json(report_path, report)
        server = OwnedServer(binary, env.copy(), evidence_directory / "product-server-0.jsonl")
        address = server.ready()
        report["product_process_database_health"] = "passed"
        report["product_gameplay_readiness"] = "awaiting_generated_protobuf_hello"
        report["current_phase"] = "real_m1_fresh_account_scenario"
        scenario_report_path = evidence_directory / "m1-fresh-account.json"
        report["journey_report"] = str(scenario_report_path.relative_to(ROOT))
        write_json(report_path, report)
        command = [
            str(client), "scenario", "m1_fresh_account", "--url", address,
            "--report", str(scenario_report_path.relative_to(ROOT)),
            "--recovery-control-dir", str(control.relative_to(ROOT)),
            "--private-checkpoint-file",
            str((control / "private-client-checkpoint.json").relative_to(ROOT)),
            "--expected-server-build", revision, "--max-seconds", str(args.max_seconds),
        ]
        with (evidence_directory / "simulator.log").open("w", encoding="utf-8") as log:
            simulator = subprocess.Popen(
                command, cwd=ROOT, env=env, stdin=subprocess.DEVNULL,
                stdout=log, stderr=subprocess.STDOUT,
            )
            deadline = time.monotonic() + args.max_seconds + 60
            handled = set()
            while simulator.poll() is None:
                require(time.monotonic() < deadline, "Owned simulator exceeded the global journey deadline.")
                for checkpoint in ("onboarding", "after_quest"):
                    request_path = control / f"restart-{checkpoint}.request.json"
                    if checkpoint in handled or not request_path.exists():
                        continue
                    request = read_json(request_path)
                    require(
                        request.get("schema_version") == 1 and request.get("checkpoint") == checkpoint
                        and request.get("origin") == server.origin
                        and request.get("require_same_isolated_database") is True,
                        "Invalid owned restart request.",
                    )
                    old_pid = server.process.pid
                    exit_code = server.stop(crash=args.restart_mode == "crash")
                    require(
                        exit_code == (-signal.SIGKILL if args.restart_mode == "crash" else 0),
                        f"Owned restart had unexpected server exit {exit_code}.",
                    )
                    if args.restart_mode == "crash":
                        time.sleep(35)  # Let the existing fenced lease expire; never edit it in SQL.
                    require(sha(binary) == report["binary_sha256"]["server"], "Server binary changed between restarts.")
                    require(game_identity(game_root) == report["game_root_identity"], "Product content changed between restarts.")
                    server = OwnedServer(binary, env.copy(), evidence_directory / f"product-server-{len(handled) + 1}.jsonl")
                    address = server.ready()
                    ack = {
                        "schema_version": 1, "request_id": request["request_id"],
                        "checkpoint": checkpoint, "status": "restarted",
                        "origin": address, "old_server_pid": old_pid,
                        "new_server_pid": server.process.pid,
                        "same_isolated_database": True, "restart_mode": args.restart_mode,
                        "server_binary_sha256": sha(binary),
                        "game_root_identity": report["game_root_identity"],
                    }
                    write_json(control / f"restart-{checkpoint}.ack.json", ack)
                    report["restarts"].append(ack)
                    handled.add(checkpoint)
                    write_json(report_path, report)
                time.sleep(0.25)
            exit_code = simulator.wait(timeout=5)
            simulator = None
        scenario = read_json(scenario_report_path)
        report["journey_report"] = str(scenario_report_path.relative_to(ROOT))
        report["scenario"] = scenario
        report["product_gameplay_readiness"] = (
            "passed" if "game.v1" in scenario.get("server_capabilities", [])
            else "unavailable"
        )
        report["unchecked_segments"] = [
            segment for segment in SEGMENTS
            if scenario.get("segments", {}).get(segment, {}).get("status") != "passed"
        ]
        require(exit_code == 0 and full_journey_passed(scenario),
                f"Real source journey blocked: {scenario.get('first_failure')}")
        require(len(report["restarts"]) == 2, "Required onboarding/post-quest owned restarts were not both executed.")
        report["status"] = "passed"
        report["full_journey_passed"] = True
    except (JourneyError, OSError, ValueError, subprocess.TimeoutExpired, urllib.error.URLError) as error:
        report["first_failure"] = {"phase": report.get("current_phase", "infrastructure"), "reason": redact(str(error), env)}
        if server is not None:
            report["server_diagnostic"] = redact(tail(server.log_path), env)
    finally:
        cleanup_errors = []
        if simulator is not None:
            if simulator.poll() is None:
                simulator.terminate()
                try:
                    simulator.wait(timeout=10)
                except subprocess.TimeoutExpired:
                    simulator.kill()
                    simulator.wait(timeout=10)
            report["owned_simulator_reaped"] = True
        if server is not None:
            try:
                record_server_exit(report, server.stop())
            except (JourneyError, OSError, subprocess.TimeoutExpired) as error:
                cleanup_errors.append(str(error))
        preserve_and_cleanup(directory, name, report, server, cleanup_errors, report_path)
    print(json.dumps({
        "status": report["status"], "full_journey_passed": report["full_journey_passed"],
        "account_checks": len(report.get("account_lifecycle", {}).get("checks", [])),
        "owned_restarts": len(report["restarts"]), "cleanup_passed": report["cleanup_passed"],
        "private_checkpoint": report["private_checkpoint"],
        "evidence": str(report_path.relative_to(ROOT)), "milestone_accepted": False,
        "first_failure": report.get("first_failure"),
    }))
    return 0 if report["status"] == "passed" and report["cleanup_passed"] else 1


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--report", default=".local/evidence/m1-headless-journey.json")
    parser.add_argument("--game-root", help="Existing complete product GameRoot, relative to this worktree.")
    parser.add_argument("--target-dir", default=".local/journey-target")
    parser.add_argument("--max-seconds", type=int, default=5400)
    parser.add_argument("--restart-mode", choices=("graceful", "crash"), default="graceful")
    parser.add_argument(
        "--server-entrypoint", choices=("binary", "public-api"), default="binary",
        help="Production binary, or the unchanged real Service through explicit Config.with_game_root.",
    )
    parser.add_argument("--skip-build", action="store_true")
    args = parser.parse_args()
    require(30 <= args.max_seconds <= 7200, "Journey timeout must be 30-7200 seconds.")
    signal.signal(signal.SIGTERM, interrupted)
    signal.signal(signal.SIGINT, interrupted)
    return run(args)


def interrupted(number, _frame):
    raise JourneyError(f"Owned orchestration interrupted by signal {number}; cleaning owned resources.")


if __name__ == "__main__":
    try:
        sys.exit(main())
    except (JourneyError, OSError, ValueError) as error:
        print(f"Journey orchestration failed: {error}", file=sys.stderr)
        sys.exit(1)

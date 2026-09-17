#!/usr/bin/env python3
"""Exact saved52 gate and reader. Implementation/preparation records grant no access."""

from __future__ import annotations

import argparse
from dataclasses import dataclass
import hashlib
import json
import os
from pathlib import Path
import re
import signal
import stat
import subprocess
import time
import uuid

import private_checkpoint as PRIVATE
import run as JOURNEY


ROOT = JOURNEY.ROOT
WORKTREE = ".worktrees/m1-saved-water-continuation"
BRANCH = "task/m1-saved-water-continuation"
SOURCE_WORKTREE = ".worktrees/m1-headless-journey"
CHECKPOINT = ".local/journey-checkpoints/52f32d29b5dc4f1a"
RUN = "52f32d29b5dc4f1a"
FROM = "5e0aa8a28851752ae0b8b0a08c635c6c8d2979f509f3a3e564ee74e2abfc8e6f"
TO = "b2a1be20a0e6c3e1968f6f7610198ce38212f196c005539b4ac5013d10ebc650"
FROM_REVISION = "m1.source-backed.v4.3ff4292b311453cc"
TO_REVISION = FROM_REVISION + ".water.e337c77cc5ccd980"
ARCHIVE = "a90bf263bcaa89dcbd4773154a5c0ff27a62de42f9762b2d6e6dfce810be6860"
INVENTORY = "a384a9ac7f8036214eb4e4bbe14a82976e33140def7a4f3239ec0079d98f570c"
IDENTITY = "3eb17ef511523a44a01e3d18a13fa00e2872e3b0a74bada5534b8eccdc511bb1"
WORLD = "95acc818-e1d6-4ad3-805f-d323623d2933"
ACCOUNT = "f43da325-ffcc-4753-a823-7a345dfce33b"
ACTOR = "actor.05a9c9bae95142fabe29d3ec35e8cf9e"
ADMISSION_PATH = "milestones/evidence/m1-saved-water-execution-02.json"
REVIEW_PATH = "milestones/evidence/m1-saved-water-continuation-review.json"
OUTPUT = ".local/saved-water-continuation-02"
JOURNAL = OUTPUT + "/authority-v1.json"
OWNER_APPROVAL = "milestones/approvals/m1-saved-water-continuation-02.json"
OWNER_APPROVAL_HASH = "facac6a31f526be54bb40dd670208d1a7bc808f792df49c1bdbd8d3dc7193495"
MANIFEST = "content/m1/legacy5e-water/manifest.json"
MANIFEST_HASH = "718d50eb2134f76b9bb2d34ba2b99f162cb2ec6666b392aba52ce2a03477cdf4"
DELIVERY = ".local/saved-water-delivery-01/game-root"
DESCRIPTOR = "24fec6e152cf5e5a899248bfe302fc12f6f080b71d43f9c0052264f1d9fcbc6e"
ASSETS = "95d7afc9d2740299c6e5091b547dbcfbc1b7b2c6f5614eca0b2251adc60abf08"
DELIVERY_INDEX = "milestones/evidence/m1-saved-water-delivery/index.json"
DELIVERY_INDEX_HASH = "a7b09591841ecda261b76d5ab4612d3ad1c15ea649da02bb39506513e07f898b"
DELIVERY_FILES = "milestones/evidence/m1-saved-water-delivery/delivery-files.json"
DELIVERY_FILES_HASH = "b4eeee21e9bf45db2a104ca1110305155069632bb371c8a563d9cb7ee7b75cb8"
PUBLIC_REPORT = "milestones/evidence/m1-cook-reward/52f32d29b5dc4f1a.json.gz"
PUBLIC_REPORT_HASH = "8e1d2bb00d120b178dcb2247cae661d151b72325d5d9813ef1217149122bb308"
PUBLIC_INPUTS = (
    "milestones/approvals/m1-water-fill-upgrade-v1.json",
    OWNER_APPROVAL,
    "milestones/evidence/m1-cook-reward-frontier.json",
    "milestones/evidence/m1-saved-water-delivery-preparation.json",
    "milestones/evidence/m1-water-main-integration.json",
    "milestones/evidence/m1-water-upgrade-preflight-receipt.json",
    "milestones/evidence/m1-water-main-integration/root-qualification/index.json",
    DELIVERY_INDEX, DELIVERY_FILES, PUBLIC_REPORT,
)
BINARY_PATHS = {
    "migrator": ".local/saved-water-target/debug/clubscape-server",
    "server": ".local/saved-water-target/debug/clubscape-journey-server",
    "simulator": ".local/saved-water-target/debug/clubscape-sim",
}
BOUNDS = {
    "attempts": 1, "restores": 1, "migrate_ui_invocations": 1,
    "databases": 1, "server_starts": 2, "postquest_restarts": 1,
    "native_preflights": 1, "native_gameplay_invocations": 1,
    "native_preflight_seconds": 60, "scenario_seconds": 600,
    "new_world_inputs_including_duplicates": 128, "migration_seconds": 35,
    "restore_seconds": 300, "database_command_seconds": 90,
    "archive_command_seconds": 300, "total_seconds": 2400,
    "preservation_cleanup_seconds": 960,
    "builds": 0, "source_generation": 0, "operator_preflight_invocations": 0,
    "browser_gpu_runelite_invocations": 0,
}
TARGET_IDENTITY = {
    "world_id": WORLD, "artifact_sha256": TO,
    "descriptor_sha256": DESCRIPTOR, "assets_manifest_sha256": ASSETS,
}


def require(condition, code):
    PRIVATE.require(condition, "saved_water", code)


def sha_bytes(value):
    return hashlib.sha256(value).hexdigest()


def hex_value(value, length=64):
    return isinstance(value, str) and re.fullmatch(rf"[0-9a-f]{{{length}}}", value) is not None


def relative(value):
    require(isinstance(value, str), "invalid_relative_path")
    path = Path(value)
    require(not path.is_absolute() and bool(path.parts) and str(path) == value
            and all(part not in {".", ".."} for part in path.parts),
            "invalid_relative_path")
    return path


def git(root, arguments):
    result = subprocess.run(
        ["git", "--no-pager", *arguments], cwd=root, stdin=subprocess.DEVNULL,
        capture_output=True, check=False, timeout=15,
    )
    require(result.returncode == 0 and len(result.stdout) <= PRIVATE.MAX_METADATA,
            "public_git_identity_unavailable")
    return result.stdout


def public_file(root, name, expected, *, maximum=64 * 1024 * 1024):
    require(hex_value(expected), "invalid_public_file_hash")
    path = PRIVATE.project_path(root, relative(name))
    metadata = path.lstat()
    require(stat.S_ISREG(metadata.st_mode) and metadata.st_size <= maximum,
            "invalid_public_file")
    require(PRIVATE.digest(path) == expected, "public_file_identity_changed")
    return path


def pinned_record(canonical, revision, name, expected):
    require(hex_value(revision, 40), "invalid_public_admission_revision")
    data = git(canonical, ["show", f"{revision}:{name}"])
    require(sha_bytes(data) == expected, "public_admission_commit_mismatch")
    path = public_file(canonical, name, expected)
    require(path.read_bytes() == data, "canonical_public_record_not_pinned")
    return json.loads(data)


def expected_start(canonical):
    # Constructing these lexical names must not stat or resolve the protected tree.
    source = canonical / SOURCE_WORKTREE
    return {
        "source_root": str(source), "checkpoint_root": str(source / CHECKPOINT),
        "run_id": RUN, "archive_sha256": ARCHIVE, "private_inventory_sha256": INVENTORY,
        "private_identity_sha256": IDENTITY, "source_artifact_sha256": FROM,
        "world_id": WORLD, "account_id": ACCOUNT, "actor_id": ACTOR, "next_sequence": 499,
    }


def expected_target(canonical):
    return {
        **TARGET_IDENTITY, "game_root": str(canonical / DELIVERY),
        "manifest": MANIFEST, "manifest_sha256": MANIFEST_HASH,
        "delivery_index_sha256": DELIVERY_INDEX_HASH,
        "delivery_inventory_sha256": DELIVERY_FILES_HASH,
    }


@dataclass(frozen=True)
class Admission:
    root: Path
    canonical: Path
    revision: str
    sha256: str
    record: dict


@dataclass(frozen=True)
class Reservation:
    admission: Admission
    run_id: str
    journal_sha256: str

    @property
    def root(self):
        return self.admission.root

    @property
    def directory(self):
        return self.root / OUTPUT

    def verify(self):
        path = PRIVATE.project_path(self.root, JOURNAL)
        value = PRIVATE.read_json(path, private=True)
        require(PRIVATE.digest(path) == self.journal_sha256
                and value == reservation_value(self.admission, self.run_id),
                "permanent_reservation_changed")

    def phase(self, name, value):
        require(re.fullmatch(r"[a-z][a-z0-9_-]{0,63}", name) is not None,
                "invalid_phase_name")
        self.verify()
        path = self.directory / f"{name}.json"
        PRIVATE.write_json(path, value)
        PRIVATE.sync_directory(self.directory)
        return path


def verify_admission(revision, expected_hash, executor_id, *, root=ROOT):
    """Only public paths are inspected here, including on every denial."""
    require(hex_value(expected_hash) and isinstance(executor_id, str)
            and re.fullmatch(r"[a-zA-Z0-9_-]{1,80}", executor_id) is not None,
            "explicit_admission_and_executor_required")
    common = Path(git(root, [
        "rev-parse", "--path-format=absolute", "--git-common-dir",
    ]).decode().strip())
    require(common.is_absolute() and common.name == ".git", "not_canonical_git_repository")
    canonical = common.parent
    require(root == canonical / WORKTREE
            and Path(git(root, ["rev-parse", "--show-toplevel"]).decode().strip()) == root
            and git(root, ["branch", "--show-current"]).decode().strip() == BRANCH,
            "wrong_exact_executor_worktree")
    require(PRIVATE.project_path(canonical, WORKTREE) == root, "symlinked_executor")
    record = pinned_record(canonical, revision, ADMISSION_PATH, expected_hash)
    require(record.get("schema_version") == 1
            and record.get("kind") == "m1_saved52_water_execution"
            and record.get("status") == "admitted"
            and record.get("authority") == "director"
            and record.get("milestone") == "m1-starter-journey"
            and record.get("saved_execution_admitted") is True
            and record.get("checkpoint_access_admitted") is True
            and record.get("automatic_resume") is False
            and record.get("milestone_accepted") is False
            and record.get("later_milestone_authorized") is False,
            "separate_saved_execution_admission_required")
    head = git(root, ["rev-parse", "HEAD"]).decode().strip()
    require(record["canonical_git_root"] == str(canonical)
            and record["executor"] == {
                "id": executor_id, "root": str(root), "worktree": WORKTREE,
                "branch": BRANCH, "code_revision": head,
            }
            and not git(root, ["status", "--porcelain", "--untracked-files=normal"]).strip(),
            "executor_code_or_identity_not_reviewed")
    require(record["start"] == expected_start(canonical)
            and record["target"] == expected_target(canonical)
            and record["bounds"] == BOUNDS
            and record["journal"] == str(root / JOURNAL)
            and record["output"] == str(root / OUTPUT),
            "exact_start_target_or_allowance_changed")
    review = record["independent_review"]
    require(review["path"] == REVIEW_PATH, "separate_independent_review_required")
    reviewed = pinned_record(canonical, review["revision"], REVIEW_PATH, review["sha256"])
    require(reviewed.get("status") == "passed"
            and reviewed.get("kind") == "m1_saved_water_continuation_code_review"
            and reviewed.get("code_revision") == head
            and reviewed.get("reviewer") != executor_id
            and isinstance(reviewed.get("reviewer"), str) and reviewed["reviewer"]
            and reviewed.get("saved_execution_admitted") is False,
            "independent_review_not_for_this_code")
    require(set(record["public_inputs"]) == set(PUBLIC_INPUTS),
            "incomplete_public_prerequisite_pins")
    require(record["public_inputs"][OWNER_APPROVAL] == OWNER_APPROVAL_HASH,
            "successor_owner_approval_not_pinned")
    for name in PUBLIC_INPUTS:
        public_file(canonical, name, record["public_inputs"][name])
    require(record["public_inputs"][DELIVERY_INDEX] == DELIVERY_INDEX_HASH
            and record["public_inputs"][DELIVERY_FILES] == DELIVERY_FILES_HASH
            and record["public_inputs"][PUBLIC_REPORT] == PUBLIC_REPORT_HASH,
            "different_public_frontier_or_delivery")
    public_file(root, MANIFEST, MANIFEST_HASH)
    require(set(record["binaries"]) == set(BINARY_PATHS), "incomplete_binary_pins")
    for name, path in BINARY_PATHS.items():
        selected = record["binaries"][name]
        require(selected["path"] == path, "different_executor_binary_path")
        binary = public_file(root, path, selected["sha256"], maximum=512 * 1024 * 1024)
        require(binary.stat().st_mode & 0o111, "binary_not_executable")
    return Admission(root, canonical, revision, expected_hash, record)


def reservation_value(admission, run_id):
    return {
        "schema_version": 1, "kind": "permanent_saved52_water_reservation",
        "admission_revision": admission.revision, "admission_sha256": admission.sha256,
        "executor": admission.record["executor"], "start": admission.record["start"],
        "target": admission.record["target"], "bounds_reserved": BOUNDS,
        "run_id": run_id, "automatic_retry": False, "automatic_resume": False,
    }


def reserve(admission):
    verified = verify_admission(admission.revision, admission.sha256,
                                admission.record["executor"]["id"], root=admission.root)
    require(verified == admission, "admission_must_be_publicly_verified_before_reservation")
    local = PRIVATE.project_path(admission.root, ".local")
    PRIVATE.private_directory(local)
    directory = PRIVATE.project_path(admission.root, OUTPUT)
    PRIVATE.private_directory(directory, new=True)
    run_id = uuid.uuid4().hex[:16]
    path = directory / "authority-v1.json"
    PRIVATE.write_json(path, reservation_value(admission, run_id))
    PRIVATE.sync_directory(directory)
    PRIVATE.sync_directory(local)
    return Reservation(admission, run_id, PRIVATE.digest(path))


def existing_reservation(admission, run_id):
    require(hex_value(run_id, 16), "invalid_reserved_run")
    path = PRIVATE.project_path(admission.root, JOURNAL)
    value = PRIVATE.read_json(path, private=True)
    require(value == reservation_value(admission, run_id), "different_permanent_reservation")
    reservation = Reservation(admission, run_id, PRIVATE.digest(path))
    reservation.verify()
    return reservation


def claim_native(admission, run_id, mode):
    require(mode in {"preflight", "gameplay"}, "invalid_native_phase")
    reservation = existing_reservation(admission, run_id)
    started = PRIVATE.read_json(reservation.directory / f"native_{mode}_started.json", private=True)
    require(started == {"run_id": run_id, "mode": mode, "admission_sha256": admission.sha256},
            "native_invocation_not_started_by_orchestrator")
    if mode == "gameplay":
        migrated = PRIVATE.read_json(reservation.directory / "migration_proved.json", private=True)
        require(migrated.get("status") == "proved"
                and migrated.get("from_artifact") == FROM and migrated.get("to_artifact") == TO
                and migrated.get("complete_private_conservation") is True
                and migrated.get("restored_private_identity_equal") is True,
                "migration_must_be_proved_before_native_gameplay")
    reservation.phase(f"native_{mode}_claimed", {
        "run_id": run_id, "mode": mode, "admission_sha256": admission.sha256,
        "automatic_retry": False,
    })
    controls = PRIVATE.read_json(reservation.directory / "controls_prepared.json", private=True)
    require(controls["run_id"] == run_id and controls["original_inventory_sha256"] == INVENTORY
            and set(controls["files"]) == {
                "resume-client-checkpoint.json", "resume-report.json", "resume-trace.jsonl",
            }, "original_control_copies_not_bound")
    for name, expected in controls["files"].items():
        path = PRIVATE.project_path(admission.root, f".local/journey-runs/{run_id}/control/{name}")
        metadata = path.lstat()
        require(stat.S_ISREG(metadata.st_mode) and metadata.st_uid == os.getuid()
                and stat.S_IMODE(metadata.st_mode) == 0o600
                and metadata.st_size <= PRIVATE.MAX_EVIDENCE
                and PRIVATE.digest(path) == expected, "original_control_copy_changed")
    return {"status": "claimed", "mode": mode, "run_id": run_id,
            "from_artifact": FROM, "to_artifact": TO, "network_operations": 0,
            "server_sha256": admission.record["binaries"]["server"]["sha256"]}


@dataclass(frozen=True)
class Saved52:
    directory: Path
    availability: dict
    inventory: dict
    capsule: dict
    orchestrator: dict
    identity: dict
    scenario: dict


def verify_boundary(available, capsule, original, identity, scenario, public_scenario):
    require(scenario == public_scenario and original["scenario"] == scenario,
            "saved_report_differs_from_exact_public_frontier")
    state = scenario["last_snapshot"]
    player = state["player"]
    source = scenario["identity"]
    world = identity["database"]["world"]
    require(source["content_artifact"]["uncompressed_sha256"] == FROM
            and source["content_revision"] == FROM_REVISION
            and capsule["schema_version"] == 1
            and capsule["kind"] == "private_m1_client_checkpoint"
            and capsule["scenario"] == "m1_fresh_account"
            and capsule["source_identity"] == source
            and capsule["last_observed_state"] == state
            and capsule["actor_id"] == ACTOR
            and capsule["private_authentication_do_not_publish"]["account_id"] == ACCOUNT
            and capsule["automatic_restore"] is False and capsule["resume_authorized"] is False
            and scenario["status"] == "blocked" and scenario["full_journey_passed"] is False
            and scenario["current_action"] == "cooks.reward_range_actual_recipe"
            and scenario["checks_passed"] == 377 and scenario["observation_checks_passed"] == 3
            and len(scenario["tutorial_edges_passed"]) == 70
            and state["tick"] == 3280 and state["revision"] == 3794
            and state["next_sequence"] == 499
            and player["actor_id"] == ACTOR and player["region"] == "region.osrs.12850"
            and player["tile"] == {"x": 3208, "y": 3216, "plane": 0}
            and player["instance"] is None
            and player["quests"]["quest.cooks_assistant"] == "stage.cooks.completed"
            and player["skills"]["skill.cooking"] == {"xp_tenths": 3000, "base_level": 4}
            and player["quest_points"] == 2,
            "not_the_exact_saved52_boundary")
    inventory = [(row["stack"]["item"], row["stack"]["quantity"]) for row in player["inventory"]]
    require(sorted(inventory) == [("item.bucket", 1), ("item.flour.pot", 1)],
            "owned_flour_and_bucket_required_no_reacquisition")
    for segment in JOURNEY.SEGMENTS:
        require(scenario["segments"][segment]["status"] == (
            "unchecked" if segment in {"cooks_reward_and_range", "after_quest_recovery"} else "passed"
        ), "completed_segment_or_remaining_scope_changed")
    require(world["world_id"] == WORLD and world["account_id"] == ACCOUNT
            and world["actor_id"] == ACTOR and world["last_sequence"] == 498
            and world["tick"] >= state["tick"] and world["revision"] >= state["revision"]
            and world["artifact_sha256"] == FROM and world["content_revision"] == FROM_REVISION
            and identity["game_root_identity"]["world_id"] == WORLD
            and identity["game_root_identity"]["artifact_sha256"] == FROM
            and identity["database_archive_sha256"] == ARCHIVE
            and available["source_artifact_sha256"] == FROM,
            "saved_private_identity_differs_from_frontier")
    latest = capsule["latest_attempt"]
    require(latest is not None and latest["sequence"] == 498
            and latest["observed_response"] == {
                "kind": "acknowledgment_received", "duplicate": False, "next_sequence": 499,
            }, "latest_operation_not_definitively_acknowledged")
    matched = [row for row in identity["database"]["selected_actual_receipts"]
               if row["operation_id"] == latest["operation_id"]]
    require(len(matched) == 1 and matched[0]["sequence"] == 498,
            "latest_acknowledgment_missing_from_private_receipts")
    control = capsule["latest_control_request"]
    require(control is None or (
        type(control["observed_http_status"]) is int
        and 200 <= control["observed_http_status"] < 300 and control["observed_error"] is None
    ), "unknown_control_outcome_no_automatic_retry")


def read_saved52(reservation):
    admission = reservation.admission
    require(verify_admission(admission.revision, admission.sha256,
                             admission.record["executor"]["id"], root=admission.root) == admission,
            "admission_changed_before_protected_read")
    reservation.phase("protected_read_started", {
        "run_id": reservation.run_id, "admission_sha256": reservation.admission.sha256,
        "exact_original_checkpoint_only": True,
    })
    canonical = reservation.admission.canonical
    directory = PRIVATE.project_path(canonical, SOURCE_WORKTREE + "/" + CHECKPOINT)
    for parent in (directory.parent, directory):
        metadata = parent.lstat()
        require(stat.S_ISDIR(metadata.st_mode) and metadata.st_uid == os.getuid()
                and stat.S_IMODE(metadata.st_mode) == 0o700, "checkpoint_directory_not_private")
    available = PRIVATE.read_json(directory / "availability.json", private=True)
    require(available["schema_version"] == 1 and available["run_id"] == RUN
            and available["directory"] == CHECKPOINT
            and available["status"] == "available" and available["snapshot_available"] is True
            and available["database_archive"] == {
                "path": "world.pgcustom", "bytes": 239240, "sha256": ARCHIVE,
            }
            and available["private_inventory_sha256"] == INVENTORY
            and available["source_artifact_sha256"] == FROM
            and available["resume_authorized"] is False
            and available["metadata_before_after_equal"] is True
            and available["archive_integrity_verified"] is True,
            "original_checkpoint_availability_changed")
    inventory_path = PRIVATE.project_path(directory, "private-inventory.json")
    inventory = PRIVATE.read_json(inventory_path, private=True)
    require(PRIVATE.digest(inventory_path) == INVENTORY,
            "original_private_inventory_changed")
    records = inventory["files"]
    require(inventory["schema_version"] == 1 and len(records) == 58
            and len({row["path"] for row in records}) == 58
            and sum(row["bytes"] for row in records) <= PRIVATE.MAX_CHECKPOINT,
            "invalid_original_file_inventory")
    required = {
        "world.pgcustom", "private-client.json", "private-identity.json", "private-service.json",
        "private-orchestrator-before-cleanup.json", "evidence/scenario.json",
        "evidence/scenario.trace.jsonl", "game-root/clubscape-game.json",
        "game-root/clubscape-game-assets.json",
    }
    require(required <= {row["path"] for row in records}, "incomplete_original_file_inventory")
    for row in records:
        path = PRIVATE.project_path(directory, relative(row["path"]))
        metadata = path.lstat()
        require(stat.S_ISREG(metadata.st_mode) and metadata.st_uid == os.getuid()
                and stat.S_IMODE(metadata.st_mode) == 0o600
                and 0 <= row["bytes"] == metadata.st_size
                and hex_value(row["sha256"]) and PRIVATE.digest(path) == row["sha256"],
                "original_private_file_changed")
    require(PRIVATE.digest(directory / "world.pgcustom") == ARCHIVE
            and PRIVATE.digest(directory / "private-identity.json") == IDENTITY,
            "original_archive_or_private_identity_changed")
    capsule = PRIVATE.read_json(directory / "private-client.json", private=True)
    original = PRIVATE.read_json(directory / "private-orchestrator-before-cleanup.json", private=True)
    identity = PRIVATE.read_json(directory / "private-identity.json", private=True)
    scenario = PRIVATE.read_json(directory / "evidence/scenario.json", 64 * 1024 * 1024, private=True)
    require(identity["client_capsule_sha256"] == PRIVATE.digest(directory / "private-client.json")
            and identity["service_config_sha256"] == PRIVATE.digest(directory / "private-service.json"),
            "capsule_or_service_identity_mismatch")
    public = JOURNEY.read_json(public_file(canonical, PUBLIC_REPORT, PUBLIC_REPORT_HASH))
    verify_boundary(available, capsule, original, identity, scenario, public["scenario"])
    reservation.phase("protected_read_verified", {
        "archive_sha256": ARCHIVE, "private_inventory_sha256": INVENTORY,
        "private_identity_sha256": IDENTITY, "private_payloads_published": False,
    })
    return Saved52(directory, available, inventory, capsule, original, identity, scenario)


def main():
    started_at = time.monotonic()
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--admission-revision", required=True)
    parser.add_argument("--admission-sha256", required=True)
    parser.add_argument("--executor-id", required=True)
    parser.add_argument("--claim-native", choices=("preflight", "gameplay"))
    parser.add_argument("--run-id")
    args = parser.parse_args()
    admission = verify_admission(args.admission_revision, args.admission_sha256, args.executor_id)
    if args.claim_native:
        print(json.dumps(claim_native(admission, args.run_id, args.claim_native)))
        return 0
    require(args.run_id is None, "run_ids_are_exclusively_reserved_not_selected")
    reservation = reserve(admission)
    import saved_water_runtime
    signal.signal(signal.SIGTERM, JOURNEY.interrupted)
    signal.signal(signal.SIGINT, JOURNEY.interrupted)
    return saved_water_runtime.execute(reservation, started_at=started_at)


def cli():
    try:
        return main()
    except (PRIVATE.CheckpointError, OSError, ValueError, KeyError, TypeError,
            subprocess.TimeoutExpired) as error:
        print(json.dumps({
            "status": "blocked", "phase": "saved_water_gate",
            "reason": error.code if isinstance(error, PRIVATE.CheckpointError) else type(error).__name__,
            "saved_execution_performed": False, "automatic_resume": False,
            "private_payloads_published": False, "milestone_accepted": False,
        }))
        return 1

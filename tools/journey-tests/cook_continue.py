"""The original unused one-shot Cook allowance, now executed by the Director."""

import json
from pathlib import Path
import subprocess

import journey_contract
import private_checkpoint as PRIVATE


AUTHORITY = "a913bb79d79e555e9c50b710e360810d95eccf4f"
AUTHORITY_PATH = "milestones/evidence/m1-cook-continuation-authorization.json"
TAKEOVER = "225045533911ed7cd326217ebff020d1e5aa7b7e"
TAKEOVER_PATH = "milestones/evidence/m1-cook-director-takeover.json"
REPAIR = "3480b6f85865827637ea0de9c30cb410f38f85c8"
REMAINING = (
    "cooks_legitimate_acquisition_partial_delivery",
    "cooks_reward_and_range",
    "after_quest_recovery",
)


def git_output(root, arguments):
    result = subprocess.run(["git", *arguments], cwd=root, capture_output=True,
                            check=False, timeout=15)
    PRIVATE.require(result.returncode == 0, "cook_authority", "pinned_record_unavailable")
    return result.stdout


def pinned_record(root, revision, path):
    return json.loads(git_output(root, ["show", f"{revision}:{path}"]))


def verify_execution_root(root, authority):
    common = Path(git_output(root, [
        "rev-parse", "--path-format=absolute", "--git-common-dir",
    ]).decode().strip())
    PRIVATE.require(root.resolve() == (common.parent / authority["execution_root"]).resolve(),
                    "cook_authority", "different_execution_root")


def verify_repair(root):
    for path in ("crates/world-engine/src/actions.rs",
                 "crates/world-engine/src/permissions.rs",
                 "tools/simulator/src/journey/source.rs"):
        PRIVATE.require((root / path).read_bytes() == git_output(root, ["show", f"{REPAIR}:{path}"]),
                        "cook_authority", "reviewed_ground_repair_changed")


def boundary(root, directory, available, capsule, scenario, identity):
    authority = pinned_record(root, AUTHORITY, AUTHORITY_PATH)
    takeover = pinned_record(root, TAKEOVER, TAKEOVER_PATH)
    start = authority["required_start"]
    world = identity["database"]["world"]
    state = scenario["last_snapshot"]
    player = state["player"]
    PRIVATE.require(authority["status"] ==
                    "same_account_accepted_cook_ingredients_reward_and_postquest_continuation_authorized"
                    and authority["bounds"]["actual_archive_restore_invocations"] == 1
                    and authority["bounds"]["scenario_seconds"] == 5400
                    and takeover["authority_revision"] == AUTHORITY
                    and takeover["new_executor"] == "director"
                    and takeover["new_or_additional_restore_budget"] is False
                    and takeover["execution"]["same_one_attempt_key"] == AUTHORITY,
                    "cook_authority", "scope_or_single_attempt_key_changed")
    verify_execution_root(root, authority)
    verify_repair(root)
    PRIVATE.require(str(directory.relative_to(root)) == start["checkpoint"]
                    and available["database_archive"]["sha256"] == start["archive_sha256"]
                    and available["private_inventory_sha256"] == start["private_inventory_sha256"]
                    and PRIVATE.digest(directory / "private-identity.json") == start["private_identity_sha256"]
                    and available["source_artifact_sha256"] == start["source_artifact_sha256"]
                    and world["world_id"] == start["world_id"]
                    and world["account_id"] == start["account_id"]
                    and world["actor_id"] == start["actor_id"]
                    and world["last_sequence"] == start["last_sequence"],
                    "cook_authority", "protected_start_identity_changed")
    PRIVATE.require(scenario["status"] == "blocked"
                    and scenario["full_journey_passed"] is False
                    and scenario["current_action"] == "cooks.actual_pot_cellar_bucket_dairy_milk"
                    and scenario["first_failure"]["action"] == scenario["current_action"]
                    and scenario["checks_passed"] == start["historical_source_checks"]
                    and len(scenario["tutorial_edges_passed"]) == start["historical_tutorial_edges"]
                    and scenario["observation_checks_passed"] == start["separate_observation_checks"]
                    and state["tick"] == start["last_public_tick"]
                    and state["revision"] == start["last_public_revision"]
                    and state["next_sequence"] == start["next_sequence"]
                    and player["actor_id"] == start["actor_id"]
                    and player["region"] == start["last_public_region"]
                    and player["tile"] == start["last_public_tile"]
                    and player["instance"] == start["last_public_instance"]
                    and player["active_death"] == start["active_death_record"]
                    and player["quests"]["quest.cooks_assistant"] == start["cook_stage"]
                    and player["skills"]["skill.cooking"]["xp_tenths"] == start["cooking_xp_tenths"]
                    and player["quest_points"] == start["quest_points"]
                    and len(player["inventory"]) == start["inventory_entries"]
                    and capsule["last_observed_state"] == state
                    and capsule["latest_attempt"]["sequence"] == start["last_sequence"]
                    and capsule["latest_attempt"]["observed_response"]["kind"] == "acknowledgment_received"
                    and all(scenario["segments"][name]["status"] ==
                            ("unchecked" if name in REMAINING else "passed")
                            for name in journey_contract.SEGMENTS),
                    "cook_authority", "not_the_unsubmitted_ingredient_boundary")
    return authority


def validate_native_preflight(value):
    PRIVATE.require(value.get("status") == "validated"
                    and value.get("network_operations") == 0 and value.get("world_inputs") == 0
                    and value["cook_boundary"]["authority"] == AUTHORITY
                    and value["cook_boundary"]["latest_acknowledged_sequence"] == 355
                    and value["cook_boundary"]["completed_segments_replayed"] is False
                    and value["cook_boundary"]["decoded_original_control"]["decoded_command"] == "poll_world"
                    and value["cook_boundary"]["decoded_original_control"]["observed_http_status"] == 200
                    and value["cook_boundary"]["decoded_original_control"]["failed_control_exception_used"] is False,
                    "cook_preflight", "original_successful_control_or_acceptance_missing")


def reserve_attempt(root, run_id, revision):
    directory = PRIVATE.project_path(root, ".local/cook-continuation-authorizations")
    PRIVATE.private_directory(directory)
    path = directory / f"{AUTHORITY}.json"
    PRIVATE.write_json(path, {
        "authority": AUTHORITY, "assignment": TAKEOVER, "executor": "director",
        "run_id": run_id, "code_revision": revision,
        "actual_restore_invocations_reserved": 1, "automatic_restore_retry": False,
    })
    PRIVATE.sync_directory(directory)
    return str(path.relative_to(root))

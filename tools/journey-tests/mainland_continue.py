"""One explicit continuation from the accepted same-account Office observation."""

import json
import subprocess

import private_checkpoint as PRIVATE


AUTHORITY = "db74895cd5d9f109d292eea20f07f5c2a57e3343"
AUTHORITY_PATH = "milestones/evidence/m1-mainland-continuation-authorization.json"


def boundary(root, directory, available, capsule, scenario, identity):
    result = subprocess.run(["git", "show", f"{AUTHORITY}:{AUTHORITY_PATH}"],
                            cwd=root, capture_output=True, text=True, check=False, timeout=15)
    PRIVATE.require(result.returncode == 0, "mainland_authority", "pinned_authority_unavailable")
    authority = json.loads(result.stdout)
    start = authority["required_start"]
    observation = authority["completed_observation"]
    player = scenario["last_snapshot"]["player"]
    PRIVATE.require(authority["status"] == "same_account_office_grave_cook_and_postquest_continuation_authorized"
                    and authority["bounds"]["actual_archive_restore_invocations"] == 1
                    and authority["bounds"]["scenario_seconds"] == 5400
                    and str(directory.relative_to(root)) == start["checkpoint"]
                    and available["database_archive"]["sha256"] == start["archive_sha256"]
                    and available["private_inventory_sha256"] == start["private_inventory_sha256"]
                    and available["source_artifact_sha256"] == start["source_artifact_sha256"]
                    and identity["database"]["world"]["world_id"] == start["world_id"]
                    and identity["database"]["world"]["actor_id"] == start["actor_id"]
                    and identity["database"]["world"]["account_id"] == start["account_id"]
                    and identity["database"]["world"]["last_sequence"] == start["last_sequence"]
                    and scenario["status"] == "observed" and scenario["observation_only"] is True
                    and scenario["dying_observation"]["successful_stop"] is True
                    and scenario["dying_observation"]["public_poll_succeeded"] is True
                    and scenario["dying_observation"]["world_inputs_emitted"] == 0
                    and scenario["checks_passed"] == start["historical_source_checks"]
                    and len(scenario["tutorial_edges_passed"]) == start["historical_tutorial_edges"]
                    and scenario["observation_checks_passed"] == start["observation_checks_are_separate"]
                    and scenario["last_snapshot"]["next_sequence"] == start["next_sequence"]
                    and player["active_death"] == observation["public_active_death"]
                    and player["instance"] == observation["public_instance"]
                    and player["region"] == observation["public_location"]["region"]
                    and player["tile"] == observation["public_location"]["tile"]
                    and capsule["last_observed_state"] == scenario["last_snapshot"],
                    "mainland_authority", "observed_success_checkpoint_mismatch")
    record = root / observation["record"]
    PRIVATE.require(PRIVATE.digest(record) == observation["record_sha256"],
                    "mainland_authority", "accepted_observation_record_changed")
    return authority


def validate_native_preflight(value):
    PRIVATE.require(value.get("status") == "validated"
                    and value.get("network_operations") == 0 and value.get("world_inputs") == 0
                    and value["mainland_boundary"]["authority"] == AUTHORITY
                    and value["mainland_boundary"]["decoded_original_control"]["decoded_command"] == "poll_world"
                    and value["mainland_boundary"]["decoded_original_control"]["observed_http_status"] == 200
                    and value["mainland_boundary"]["decoded_original_control"]["failed_control_exception_used"] is False,
                    "mainland_preflight", "original_control_not_successful_typed_poll")


def reserve_attempt(root, run_id, revision):
    directory = PRIVATE.project_path(root, ".local/mainland-continuation-authorizations")
    PRIVATE.private_directory(directory)
    path = directory / f"{AUTHORITY}.json"
    PRIVATE.write_json(path, {
        "authority": AUTHORITY, "run_id": run_id, "code_revision": revision,
        "actual_restore_invocations_reserved": 1, "automatic_restore_retry": False,
    })
    PRIVATE.sync_directory(directory)
    return str(path.relative_to(root))

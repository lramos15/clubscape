"""The single explicitly authorized restored observation, not gameplay continuation."""

import json
import os
import subprocess

import private_checkpoint as PRIVATE


AUTHORITY = "bd33c742bad7be4ab244e1402cc840d89d3e427b"
AUTHORITY_PATH = "milestones/evidence/m1-dying-observation-authorization.json"
ERROR_ID = "688aab5a-a58b-4a57-bb48-ec9dcdd1391a"
REQUEST_ID = "6ec49ce1-3987-43d4-9bc9-37afc6c6f2a3"

FACTS_SQL = """
SELECT jsonb_build_object(
  'world_tick', w.tick, 'world_revision', w.revision,
  'world_id', w.world_id, 'actor_id', c.actor_id, 'account_id', c.account_id,
  'last_sequence', c.last_sequence,
  'hitpoints', a->'hitpoints', 'region', a->'region', 'tile', a->'tile',
  'life', a->'runtime'->'life', 'instance', a->'runtime'->'instance',
  'active_death', a->'runtime'->'active_death',
  'combat_style', a->'runtime'->'combat'->'style',
  'owned_progress_sha256', encode(sha256(convert_to(jsonb_build_object(
    'inventory', a->'inventory', 'equipment', a->'equipment', 'bank', a->'bank',
    'quests', a->'quests', 'quest_points', a->'quest_points',
    'tutorial_stage', a->'tutorial_stage', 'interfaces', a->'interfaces',
    'xp', (SELECT jsonb_object_agg(key, value->'xp_tenths') FROM jsonb_each(a->'skills'))
  )::text, 'UTF8')), 'hex'),
  'death_items_sha256', encode(sha256(convert_to(jsonb_build_object(
    'retained', d->'retained', 'grave_items', d->'grave'->'items',
    'office', d->'office', 'reclaimed', d->'reclaimed', 'discarded', d->'discarded',
    'owner', d->'owner', 'occurred_at_tick', d->'occurred_at_tick'
  )::text, 'UTF8')), 'hex')
)
FROM game_worlds w JOIN game_characters c ON c.world_id=w.world_id
CROSS JOIN LATERAL (SELECT w.state->'characters'->c.actor_id AS a) actor_state
CROSS JOIN LATERAL (SELECT w.state->'runtime'->'deaths'->(a->'runtime'->>'active_death') AS d) death_state
WHERE w.world_id=:'world_id'::uuid AND c.account_id=:'account_id'::uuid;
"""


def authority(root):
    result = subprocess.run(["git", "show", f"{AUTHORITY}:{AUTHORITY_PATH}"],
                            cwd=root, capture_output=True, text=True, check=False, timeout=15)
    PRIVATE.require(result.returncode == 0, "authorization", "pinned_authority_unavailable")
    value = json.loads(result.stdout)
    PRIVATE.require(value["task"] == "M1-DYING-RESTORE-OBSERVE"
                    and value["status"] == "bounded_adapter_and_restored_observation_authorized"
                    and value["bounds"]["actual_restore_invocations"] == 1
                    and value["bounds"]["maximum_seconds_after_server_ready"] == 180
                    and value["bounds"]["player_world_inputs"] is False,
                    "authorization", "unexpected_authority")
    return value


def boundary(root, directory, available, capsule, scenario, identity):
    approved = authority(root)
    original = approved["immutable_original"]
    PRIVATE.require(str(directory.relative_to(root)) == original["checkpoint"]
                    and available["database_archive"]["sha256"] == original["archive_sha256"]
                    and available["private_inventory_sha256"] == original["private_inventory_sha256"]
                    and available["source_artifact_sha256"] == original["source_artifact_sha256"]
                    and identity["database"]["world"]["tick"] == original["world_tick"]
                    and identity["database"]["world"]["last_sequence"] == original["last_acknowledged_sequence"]
                    and scenario["last_snapshot"]["tick"] == original["last_public_tick"]
                    and scenario["last_snapshot"]["player"]["vitals"]["hitpoints"] == original["last_public_hp"]
                    and capsule["last_observed_state"] == scenario["last_snapshot"]
                    and len(scenario["tutorial_edges_passed"]) == 70
                    and scenario["checks_passed"] == 321
                    and ERROR_ID in scenario["first_failure"]["reason"],
                    "authorization", "original_checkpoint_boundary_mismatch")
    cause = root / approved["prerequisites_satisfied"]["actual_cause_record"]
    PRIVATE.require(PRIVATE.digest(cause) == approved["prerequisites_satisfied"]["actual_cause_record_sha256"],
                    "authorization", "actual_cause_proof_mismatch")
    return approved


def failed_control_candidate(control):
    return (control.get("observed_http_status") == 409
            and control.get("operation_id") == REQUEST_ID
            and control.get("observed_error") == [3, ERROR_ID])


def validate_native_preflight(value):
    PRIVATE.require(value.get("status") == "validated"
                    and value.get("network_operations") == 0 and value.get("world_inputs") == 0
                    and value["observation_boundary"]["authority"] == AUTHORITY,
                    "observation_preflight", "native_control_validation_failed")
    control = value["observation_boundary"]["original_recorded_control"]
    if control["failed_control_exception_used"]:
        PRIVATE.require(control["decoded_command"] == "poll_world"
                        and control["operation_id"] == REQUEST_ID
                        and control["observed_http_status"] == 409
                        and control["observed_error"] == [3, ERROR_ID],
                        "observation_preflight", "failed_control_not_the_typed_readonly_query")


def reserve_attempt(root, run_id, code_revision):
    directory = PRIVATE.project_path(root, ".local/dying-observation-authorizations")
    PRIVATE.private_directory(directory)
    path = directory / f"{AUTHORITY}.json"
    PRIVATE.write_json(path, {
        "authority": AUTHORITY, "run_id": run_id, "code_revision": code_revision,
        "actual_restore_invocations_reserved": 1,
        "automatic_retry": False,
    })
    PRIVATE.sync_directory(directory)
    return str(path.relative_to(root))


def saved_facts(root, directory, owner, capsule, identity, phase):
    sql = directory / "dying-facts.sql"
    if not sql.exists():
        with PRIVATE.private_file(sql) as stream:
            stream.write(FACTS_SQL.encode("ascii"))
            stream.flush()
            os.fsync(stream.fileno())
    PRIVATE.private_command(root, directory, phase, [
        "docker", "exec", "-i", owner, "psql", "--no-psqlrc", "--no-password",
        "--host=/var/run/postgresql", "--username=clubscape", "--dbname=clubscape_journey",
        "--tuples-only", "--no-align", "--set=ON_ERROR_STOP=1",
        "--set=world_id=" + identity["game_root_identity"]["world_id"],
        "--set=account_id=" + capsule["private_authentication_do_not_publish"]["account_id"],
    ], output=f"{phase}.json", stdin=sql)
    value = PRIVATE.read_json(directory / f"{phase}.json", private=True)
    PRIVATE.require(value["last_sequence"] == 301 and value["active_death"] is not None,
                    phase, "death_or_acknowledged_sequence_changed")
    return value


def compare_saved(before, after):
    PRIVATE.require(before["owned_progress_sha256"] == after["owned_progress_sha256"]
                    and before["death_items_sha256"] == after["death_items_sha256"]
                    and before["active_death"] == after["active_death"]
                    and before["last_sequence"] == after["last_sequence"] == 301,
                    "observation_preservation", "source_owned_progress_or_death_items_changed")
    PRIVATE.require(after["world_tick"] >= before["world_tick"]
                    and after["world_revision"] >= before["world_revision"],
                    "observation_preservation", "source_clock_regressed")
    return {
        "owned_items_equipment_bank_xp_quests_unchanged": True,
        "death_retention_grave_office_claim_items_unchanged": True,
        "last_acknowledged_sequence": 301, "next_sequence": 302,
        "private_saved_before_startup": {
            "tick": before["world_tick"], "hitpoints": before["hitpoints"],
            "life_phase": before["life"]["kind"], "combat_style": before["combat_style"],
        },
        "private_after_observation": {
            "tick": after["world_tick"], "hitpoints": after["hitpoints"],
            "life_phase": after["life"]["kind"], "combat_style": after["combat_style"],
        },
        "post_startup_lifecycle_and_ticks_not_claimed_unchanged": True,
    }


def validate_observation_report(scenario, original_inputs):
    result = scenario.get("dying_observation", {})
    PRIVATE.require(scenario.get("status") == "observed"
                    and scenario.get("observation_only") is True
                    and scenario.get("full_journey_passed") is False
                    and result.get("successful_stop") is True
                    and result.get("public_poll_succeeded") is True
                    and result.get("world_inputs_emitted") == 0
                    and scenario["input_count"] == original_inputs
                    and scenario["last_sequence"] == 301,
                    "observation", "incomplete_or_mutating_observation")


def success_checkpoint_eligible(scenario):
    result = scenario.get("dying_observation", {})
    return (scenario.get("status") == "observed" and scenario.get("observation_only") is True
            and result.get("authority") == AUTHORITY
            and result.get("successful_stop") is True
            and result.get("public_poll_succeeded") is True
            and result.get("world_inputs_emitted") == 0)

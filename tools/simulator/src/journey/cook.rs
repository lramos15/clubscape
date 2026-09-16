use anyhow::{Context, Result, ensure};
use clubscape_protocol::game;
use serde_json::{Value, json};

use super::{checkpoint, evidence, observation};

pub(super) const AUTHORITY: &str = "a913bb79d79e555e9c50b710e360810d95eccf4f";
const ACTOR: &str = "actor.05a9c9bae95142fabe29d3ec35e8cf9e";

#[derive(Clone, Copy)]
pub(super) struct Baseline {
    pub cooking_xp: u64,
    pub quest_points: u32,
    pub coins: u64,
}

pub(super) struct Continuation {
    pub baseline: Baseline,
    pub verification: Value,
}

pub(super) fn admitted(capsule: &Value, report: &Value) -> Result<Continuation> {
    let state = &report["last_snapshot"];
    let player = &state["player"];
    ensure!(
        capsule["source_identity"]["content_artifact"]["uncompressed_sha256"]
            == observation::SOURCE
            && capsule["last_observed_state"] == *state
            && report["status"] == "blocked"
            && report["full_journey_passed"] == false
            && report["checks_passed"] == 336
            && report["observation_checks_passed"] == 3
            && report["current_action"] == "cooks.actual_pot_cellar_bucket_dairy_milk"
            && report["first_failure"]["action"] == report["current_action"]
            && state["tick"] == 1886
            && state["revision"] == 2255
            && state["next_sequence"] == 356
            && player["actor_id"] == ACTOR
            && player["region"] == "region.osrs.12850"
            && player["tile"] == json!({"x":3209,"y":3213,"plane":0})
            && player["instance"].is_null()
            && player["active_death"] == "death.engine.1615.0"
            && player["tutorial_stage"] == "stage.tutorial.mainland"
            && player["quests"]["quest.cooks_assistant"] == "stage.cooks.delivered.none"
            && player["skills"]["skill.cooking"]["xp_tenths"] == 0
            && player["quest_points"] == 1
            && player["inventory"].as_array().is_some_and(Vec::is_empty),
        "Not the authorized accepted-Cook pre-ingredient checkpoint"
    );
    for &segment in evidence::SEGMENTS {
        let pending = matches!(
            segment,
            "cooks_legitimate_acquisition_partial_delivery"
                | "cooks_reward_and_range"
                | "after_quest_recovery"
        );
        ensure!(
            report["segments"][segment]["status"] == if pending { "unchecked" } else { "passed" },
            "Completed journey segments must not be repeated"
        );
    }
    checkpoint::Attempt::from_saved(&capsule["latest_attempt"])?.require_acknowledged_action(
        355,
        &game::world_input::Action::DialogueChoice(game::DialogueChoice {
            speaker: "spawn.cook".into(),
            choice: "transition.cooks.accept".into(),
        }),
    )?;
    let control = observation::validate_control(&capsule["latest_control_request"], false)?;
    ensure!(
        control["decoded_command"] == "poll_world"
            && control["observed_http_status"] == 200
            && control["failed_control_exception_used"] == false,
        "Cook continuation requires the recorded successful typed poll"
    );
    Ok(Continuation {
        baseline: Baseline {
            cooking_xp: player["skills"]["skill.cooking"]["xp_tenths"]
                .as_u64()
                .context("Original Cooking XP missing")?,
            quest_points: player["quest_points"]
                .as_u64()
                .context("Original quest points missing")?
                .try_into()?,
            coins: 0,
        },
        verification: json!({
            "authority": AUTHORITY, "decoded_original_control": control,
            "latest_acknowledged_sequence": 355, "next_sequence": 356,
            "historical_source_checks": 336, "separate_observation_checks": 3,
            "completed_segments_replayed": false,
            "remaining": ["cooks_legitimate_acquisition_partial_delivery", "cooks_reward_and_range", "after_quest_recovery"],
        }),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use clubscape_protocol::{ClientMessage, PROTOCOL_VERSION, client_message::Command};
    use prost::Message;

    fn fixture() -> (Value, Value) {
        let operation = "3b97702c-f9f2-4b14-8517-69d5035dfd07";
        let input = game::WorldInput {
            sequence: 355,
            action: Some(game::world_input::Action::DialogueChoice(
                game::DialogueChoice {
                    speaker: "spawn.cook".into(),
                    choice: "transition.cooks.accept".into(),
                },
            )),
            ..Default::default()
        };
        let control = ClientMessage {
            protocol_version: PROTOCOL_VERSION,
            request_id: operation.into(),
            command: Some(Command::PollWorld(Default::default())),
        };
        let state = json!({
            "tick":1886,"revision":2255,"next_sequence":356,
            "player":{"actor_id":ACTOR,"region":"region.osrs.12850",
                "tile":{"x":3209,"y":3213,"plane":0},"instance":null,
                "active_death":"death.engine.1615.0","tutorial_stage":"stage.tutorial.mainland",
                "quests":{"quest.cooks_assistant":"stage.cooks.delivered.none"},
                "skills":{"skill.cooking":{"xp_tenths":0}},"quest_points":1,"inventory":[]}
        });
        let capsule = json!({
            "source_identity":{"content_artifact":{"uncompressed_sha256":observation::SOURCE}},
            "last_observed_state":state,
            "latest_attempt":{"operation_id":operation,"sequence":355,"world_input_protobuf":input.encode_to_vec(),
                "observed_response":{"kind":"acknowledgment_received","duplicate":false,"next_sequence":356}},
            "latest_control_request":{"operation_id":operation,"client_message_protobuf":control.encode_to_vec(),
                "observed_http_status":200,"observed_error":null,"private_bearer_token":"fixture-secret"}
        });
        let mut report = json!({
            "status":"blocked","full_journey_passed":false,"checks_passed":336,
            "observation_checks_passed":3,"last_snapshot":state,
            "current_action":"cooks.actual_pot_cellar_bucket_dairy_milk",
            "first_failure":{"action":"cooks.actual_pot_cellar_bucket_dairy_milk"}
        });
        for &segment in evidence::SEGMENTS {
            let status = if matches!(
                segment,
                "cooks_legitimate_acquisition_partial_delivery"
                    | "cooks_reward_and_range"
                    | "after_quest_recovery"
            ) {
                "unchecked"
            } else {
                "passed"
            };
            report["segments"][segment] = json!({"status":status});
        }
        (capsule, report)
    }

    #[test]
    fn accepted_cook_boundary_keeps_completed_death_and_original_empty_inventory() {
        let (capsule, report) = fixture();
        let value = admitted(&capsule, &report).unwrap();
        assert_eq!(value.baseline.cooking_xp, 0);
        assert_eq!(value.baseline.quest_points, 1);
        assert_eq!(value.baseline.coins, 0);
        assert_eq!(value.verification["completed_segments_replayed"], false);
        assert!(!value.verification.to_string().contains("fixture-secret"));
        assert_eq!(
            report["segments"]["source_death_office_grave_recovery"]["status"],
            "passed"
        );
    }

    #[test]
    fn a_different_frontier_or_completed_segment_cannot_be_restarted() {
        for case in [
            "count",
            "inventory",
            "quest",
            "completed_death",
            "already_delivered",
            "snapshot",
        ] {
            let (capsule, mut report) = fixture();
            match case {
                "count" => report["checks_passed"] = json!(337),
                "inventory" => {
                    report["last_snapshot"]["player"]["inventory"] = json!([{"item":"not-a-grant"}])
                }
                "quest" => {
                    report["last_snapshot"]["player"]["quests"]["quest.cooks_assistant"] =
                        json!("stage.cooks.completed")
                }
                "completed_death" => {
                    report["segments"]["source_death_office_grave_recovery"]["status"] =
                        json!("unchecked")
                }
                "already_delivered" => {
                    report["segments"]["cooks_legitimate_acquisition_partial_delivery"]["status"] =
                        json!("passed")
                }
                "snapshot" => report["last_snapshot"]["revision"] = json!(2256),
                _ => unreachable!(),
            }
            assert!(admitted(&capsule, &report).is_err(), "{case}");
        }
    }

    #[test]
    fn cook_continuation_requires_the_original_acknowledged_acceptance_and_successful_poll() {
        for case in [
            "unresolved",
            "duplicate",
            "next_sequence",
            "different_action",
            "failed_control",
            "wrong_control",
        ] {
            let (mut capsule, report) = fixture();
            match case {
                "unresolved" => {
                    capsule["latest_attempt"]["observed_response"] = json!({"kind":"unresolved"})
                }
                "duplicate" => {
                    capsule["latest_attempt"]["observed_response"]["duplicate"] = json!(true)
                }
                "next_sequence" => {
                    capsule["latest_attempt"]["observed_response"]["next_sequence"] = json!(355)
                }
                "different_action" => {
                    let input = game::WorldInput {
                        sequence: 355,
                        action: Some(game::world_input::Action::CloseInterface(game::Empty {})),
                        ..Default::default()
                    };
                    capsule["latest_attempt"]["world_input_protobuf"] =
                        json!(input.encode_to_vec());
                }
                "failed_control" => {
                    capsule["latest_control_request"]["observed_http_status"] = json!(409)
                }
                "wrong_control" => {
                    let control = ClientMessage {
                        protocol_version: PROTOCOL_VERSION,
                        request_id: capsule["latest_control_request"]["operation_id"]
                            .as_str()
                            .unwrap()
                            .into(),
                        command: Some(Command::Logout(Default::default())),
                    };
                    capsule["latest_control_request"]["client_message_protobuf"] =
                        json!(control.encode_to_vec());
                }
                _ => unreachable!(),
            }
            assert!(admitted(&capsule, &report).is_err(), "{case}");
        }
    }
}

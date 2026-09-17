use std::{path::Path, process::Command};

use anyhow::{Context, Result, ensure};
use clubscape_protocol::{client_message, game};
use serde_json::{Value, json};

use super::{Arguments, Receipt, checkpoint, evidence, observation, source};

pub(super) const FROM: &str = observation::SOURCE;
pub(super) const TO: &str = "b2a1be20a0e6c3e1968f6f7610198ce38212f196c005539b4ac5013d10ebc650";
pub(super) const TO_REVISION: &str = "m1.source-backed.v4.3ff4292b311453cc.water.e337c77cc5ccd980";
pub(super) const MANIFEST: &str = "content/m1/legacy5e-water/manifest.json";
pub(super) const MANIFEST_HASH: &str =
    "718d50eb2134f76b9bb2d34ba2b99f162cb2ec6666b392aba52ce2a03477cdf4";
pub(super) const WORLD: &str = "95acc818-e1d6-4ad3-805f-d323623d2933";
pub(super) const ACCOUNT: &str = "f43da325-ffcc-4753-a823-7a345dfce33b";
pub(super) const ACTOR: &str = "actor.05a9c9bae95142fabe29d3ec35e8cf9e";
pub(super) const SINK: &str = "spawn.water_source.3205.3215.p0.t10.r0";
pub(super) const RANGE: &str = "spawn.range.lumbridge.3212.3215.p0.t10.r2";
pub(super) const MAX_SECONDS: u64 = 600;
pub(super) const MAX_INPUTS: u64 = 128;
pub(super) const NEXT_SEQUENCE: u64 = 499;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Phase {
    Entry,
    Water,
    Dough,
    Range,
    Recovery,
}

pub(super) struct Continuation {
    pub verification: Value,
}

pub(super) struct Progress {
    pub phase: Phase,
    pub historical_inputs: u64,
    pub receipt: Option<Receipt>,
    pub server_hash: String,
}

pub(super) fn claim_gate(arguments: &Arguments) -> Result<Value> {
    ensure!(
        arguments.source_root == Path::new(".")
            && arguments.source_manifest.as_deref() == Some(Path::new(MANIFEST))
            && arguments.max_seconds <= MAX_SECONDS
            && arguments.max_inputs <= MAX_INPUTS,
        "Saved-water requires its explicit b2 source and remaining-only bounds"
    );
    ensure!(
        std::env::current_exe()?
            == std::env::current_dir()?.join(".local/saved-water-target/debug/clubscape-sim"),
        "Saved-water must use the exact executor's pinned native binary"
    );
    let capsule = arguments
        .resume_client_checkpoint
        .as_ref()
        .context("Saved-water requires the reserved control capsule")?;
    let parts = capsule
        .iter()
        .map(|value| value.to_str())
        .collect::<Option<Vec<_>>>()
        .context("Invalid saved-water control path")?;
    ensure!(
        parts.len() == 5
            && parts[0] == ".local"
            && parts[1] == "journey-runs"
            && parts[2].len() == 16
            && parts[2]
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
            && parts[3] == "control"
            && parts[4] == "resume-client-checkpoint.json",
        "Saved-water never directly reads a checkpoint directory"
    );
    let control = capsule
        .parent()
        .context("Missing reserved control directory")?;
    ensure!(
        arguments.recovery_control_dir.as_deref() == Some(control)
            && arguments.private_checkpoint_file.as_deref()
                == Some(control.join("private-client-checkpoint.json").as_path())
            && arguments.report
                == control
                    .parent()
                    .context("Missing run directory")?
                    .join("scenario.json"),
        "Saved-water outputs must use the exact reserved run"
    );
    let revision = arguments
        .saved_water_admission_revision
        .as_deref()
        .context("Separate committed execution admission required")?;
    let hash = arguments
        .saved_water_admission_sha256
        .as_deref()
        .context("Exact execution admission hash required")?;
    let executor = arguments
        .saved_water_executor
        .as_deref()
        .context("Exact execution identity required")?;
    // The shared public gate claims a single native slot before any control-path I/O.
    let result = Command::new("python3")
        .args([
            "tools/journey-tests/saved_water.py",
            "--admission-revision",
            revision,
            "--admission-sha256",
            hash,
            "--executor-id",
            executor,
            "--claim-native",
            if arguments.validate_resume_only {
                "preflight"
            } else {
                "gameplay"
            },
            "--run-id",
            parts[2],
        ])
        .output()
        .context("Public saved-water execution gate could not run")?;
    ensure!(
        result.status.success() && result.stdout.len() <= 8192,
        "Separate saved-water execution admission/reservation was denied"
    );
    let value: Value = serde_json::from_slice(&result.stdout)?;
    ensure!(
        value["status"] == "claimed"
            && value["run_id"] == parts[2]
            && value["from_artifact"] == FROM
            && value["to_artifact"] == TO
            && value["server_sha256"].as_str().is_some_and(
                |hash| hash.len() == 64 && hash.bytes().all(|byte| byte.is_ascii_hexdigit())
            ),
        "Saved-water public gate returned a different reservation"
    );
    Ok(value)
}

pub(super) fn admitted(
    capsule: &Value,
    report: &Value,
    selected: &source::Source,
) -> Result<Continuation> {
    let state = &report["last_snapshot"];
    let player = &state["player"];
    ensure!(
        selected.identity["content_artifact"]["uncompressed_sha256"] == TO
            && capsule["source_identity"] == report["identity"]
            && report["identity"]["content_artifact"]["uncompressed_sha256"] == FROM
            && capsule["last_observed_state"] == *state
            && capsule["actor_id"] == ACTOR
            && capsule["private_authentication_do_not_publish"]["account_id"] == ACCOUNT
            && capsule["resume_authorized"] == false
            && capsule["automatic_restore"] == false
            && report["status"] == "blocked"
            && report["full_journey_passed"] == false
            && report["checks_passed"] == 377
            && report["observation_checks_passed"] == 3
            && report["input_count"] == 603
            && report["trace_records"] == 4547
            && report["tutorial_edges_passed"]
                .as_array()
                .is_some_and(|edges| edges.len() == 70)
            && report["current_action"] == "cooks.reward_range_actual_recipe"
            && report["first_failure"]["action"] == report["current_action"]
            && state["tick"] == 3280
            && state["revision"] == 3794
            && state["next_sequence"] == NEXT_SEQUENCE
            && player["actor_id"] == ACTOR
            && player["region"] == "region.osrs.12850"
            && player["tile"] == json!({"x":3208,"y":3216,"plane":0})
            && player["instance"].is_null()
            && player["tutorial_stage"] == "stage.tutorial.mainland"
            && player["quests"]["quest.cooks_assistant"] == "stage.cooks.completed"
            && player["skills"]["skill.cooking"] == json!({"xp_tenths":3000,"base_level":4})
            && player["quest_points"] == 2,
        "Not the migration-aware exact saved52 range frontier"
    );
    let mut items = player["inventory"]
        .as_array()
        .context("Saved inventory missing")?
        .iter()
        .map(|row| {
            Ok((
                row["stack"]["item"]
                    .as_str()
                    .context("Saved item missing")?,
                row["stack"]["quantity"]
                    .as_u64()
                    .context("Saved quantity missing")?,
            ))
        })
        .collect::<Result<Vec<_>>>()?;
    items.sort_unstable();
    ensure!(
        items == [("item.bucket", 1), ("item.flour.pot", 1)],
        "Saved-water cannot reacquire flour or a bucket"
    );
    for &segment in evidence::SEGMENTS {
        let pending = matches!(segment, "cooks_reward_and_range" | "after_quest_recovery");
        ensure!(
            report["segments"][segment]["status"] == if pending { "unchecked" } else { "passed" },
            "Saved-water cannot replay a completed segment"
        );
    }
    checkpoint::Attempt::from_saved(&capsule["latest_attempt"])?
        .require_acknowledged_sequence(498)?;
    let control = observation::validate_control(&capsule["latest_control_request"], false)?;
    ensure!(
        control["failed_control_exception_used"] == false,
        "No old failed-control exception"
    );
    Ok(Continuation {
        verification: json!({
            "from_artifact": FROM, "to_artifact": TO,
            "latest_acknowledged_sequence": 498, "next_sequence": NEXT_SEQUENCE,
            "historical_source_checks": 377, "historical_tutorial_edges": 70,
            "separate_observation_checks": 3, "completed_segments_replayed": false,
            "original_control": control, "old_reward_replayed": false,
            "remaining": ["saved_bucket_water", "existing_flour_dough_range", "after_quest_recovery"]
        }),
    })
}

pub(super) fn lifecycle_command(command: &client_message::Command) -> bool {
    matches!(
        command,
        client_message::Command::Hello(_)
            | client_message::Command::Login(_)
            | client_message::Command::CurrentAccount(_)
            | client_message::Command::JoinWorld(_)
            | client_message::Command::LeaveWorld(_)
            | client_message::Command::Logout(_)
    ) || matches!(command, client_message::Command::PollWorld(query) if query.quote.is_none())
}

pub(super) fn require_new_replay(receipt: &Receipt, acknowledged: Option<&Receipt>) -> Result<()> {
    let known = acknowledged.context("No newly acknowledged continuation operation")?;
    ensure!(
        receipt.sequence >= NEXT_SEQUENCE
            && receipt.operation_id == known.operation_id
            && receipt.sequence == known.sequence
            && receipt.action == known.action
            && receipt.observed_revision == known.observed_revision,
        "Only the newly acknowledged water operation may be replayed, never Cook's reward"
    );
    Ok(())
}

pub(super) fn validate_input(
    runner: &super::Runner,
    receipt: &Receipt,
    duplicate: bool,
) -> Result<()> {
    use game::{gameplay_ui_request::Request, world_input::Action};
    let water = runner
        .saved_water
        .as_ref()
        .context("Missing saved-water phase")?;
    if duplicate {
        ensure!(
            water.phase == Phase::Recovery,
            "Replay outside saved-water recovery"
        );
        return require_new_replay(receipt, water.receipt.as_ref());
    }
    ensure!(
        !matches!(water.phase, Phase::Entry | Phase::Recovery) && receipt.sequence >= NEXT_SEQUENCE,
        "Saved-water entry/recovery must be lifecycle-only"
    );
    let permitted = match &receipt.action {
        Action::Walk(_) => matches!(water.phase, Phase::Water | Phase::Range),
        Action::Interact(input) => {
            let source_action = runner.source.interaction(&input.target, &input.action)?;
            (input.action == "Open"
                && runner.source.content["mechanics"]["object_transforms"]
                    .as_object()
                    .is_some_and(|transforms| {
                        transforms
                            .values()
                            .any(|transform| transform["spawn"] == input.target)
                    })
                && matches!(water.phase, Phase::Water | Phase::Range))
                || (water.phase == Phase::Range
                    && input.target == RANGE
                    && source_action["action"]["recipes"]
                        .as_array()
                        .is_some_and(|recipes| {
                            recipes
                                .iter()
                                .any(|id| id == "recipe.cooking.bread.lumbridge_range")
                        }))
        }
        Action::UseItem(input) => match (&water.phase, &input.target) {
            (Phase::Water, Some(game::use_item::Target::WorldSpawn(target))) => {
                target == SINK
                    && input.inventory_slot == runner.slot("item.bucket")?
                    && water.receipt.is_none()
            }
            (Phase::Dough, Some(game::use_item::Target::OtherInventorySlot(other))) => {
                input.inventory_slot == runner.slot("item.flour.pot")?
                    && *other == runner.slot("item.water.bucket")?
            }
            _ => false,
        },
        Action::Ui(input) => {
            input.expected_bank_revision.is_none()
                && matches!(&input.request, Some(Request::Production(selection))
                if selection.quantity == 1 && selection.mode == game::ProductionMode::Single as i32
                    && match water.phase {
                        Phase::Dough => selection.recipe == "recipe.cooking.dough",
                        Phase::Range => selection.recipe == "recipe.cooking.bread.lumbridge_range",
                        _ => false,
                    })
        }
        Action::CancelActivity(_) => water.phase == Phase::Range,
        _ => false,
    };
    ensure!(
        permitted,
        "Input exceeds the admitted water/dough/range remainder"
    );
    Ok(())
}

pub(super) fn water_conversion(before: &Value, after: &Value) -> Result<()> {
    let mut expected = before.clone();
    let slots = expected["inventory"]
        .as_array_mut()
        .context("Missing water inventory")?;
    let buckets: Vec<_> = slots
        .iter_mut()
        .filter(|slot| slot["stack"]["item"] == "item.bucket")
        .collect();
    ensure!(buckets.len() == 1, "One existing bucket is required");
    for bucket in buckets {
        ensure!(
            bucket["stack"]["quantity"] == 1,
            "Water must convert only one bucket"
        );
        bucket["stack"]["item"] = json!("item.water.bucket");
    }
    ensure!(
        expected == *after,
        "Water changed more than the one owned bucket"
    );
    Ok(())
}

impl super::Runner {
    pub(super) async fn continue_saved_water(&mut self, resume: checkpoint::Resume) -> Result<()> {
        self.evidence.report["saved_water_boundary"] = resume
            .saved_water
            .as_ref()
            .context("Exact saved-water boundary missing")?
            .verification
            .clone();
        self.account_id = resume.string("/private_authentication_do_not_publish/account_id")?;
        self.login_name = resume.string("/private_authentication_do_not_publish/login_name")?;
        self.password = resume.string("/private_authentication_do_not_publish/password")?;
        self.actor_id = resume.string("/actor_id")?;
        self.sequence = NEXT_SEQUENCE;
        self.onboarding_receipt = resume.receipt("onboarding")?;
        self.reward_receipt = resume.receipt("reward")?;
        self.private_attempt = Some(checkpoint::Attempt::from_saved(
            &resume.capsule["latest_attempt"],
        )?);
        self.private_control =
            checkpoint::ControlAttempt::from_saved(&resume.capsule["latest_control_request"])?;
        self.historical_observation = Some(resume.capsule["last_observed_state"].clone());
        self.label("saved_water.lifecycle_only_entry")?;
        self.hello().await?;
        self.login().await?;
        self.join().await?;
        self.evidence.check(
            "saved_water_original_next_sequence",
            json!(NEXT_SEQUENCE),
            json!(self.sequence),
        )?;
        let actual = evidence::stable_player(self.player()?);
        let expected = actual
            .as_object()
            .context("Invalid stable player")?
            .keys()
            .map(|key| {
                Ok((
                    key.clone(),
                    resume.capsule["last_observed_state"]["player"]
                        .get(key)
                        .with_context(|| format!("Original public player field missing: {key}"))?
                        .clone(),
                ))
            })
            .collect::<Result<serde_json::Map<_, _>>>()?;
        self.evidence.check(
            "saved_water_lifecycle_entry_preserved_player",
            Value::Object(expected),
            actual,
        )?;
        let progress = self
            .saved_water
            .as_ref()
            .context("Missing saved-water progress")?;
        ensure!(
            self.input_count == progress.historical_inputs && progress.phase == Phase::Entry,
            "WorldInput was submitted before lifecycle-only restoration was verified"
        );
        self.evidence.report["saved_water_lifecycle_only_entry"] = json!(true);
        self.saved_water_range().await?;
        self.saved_water_recovery().await?;
        self.logout_lifecycle().await?;
        for &segment in evidence::SEGMENTS {
            if resume.report["segments"][segment]["status"] == "passed" {
                ensure!(
                    self.evidence.report["segments"][segment] == resume.report["segments"][segment],
                    "A completed historical segment was rewritten"
                );
            }
        }
        self.evidence.report["saved_water_new_input_count"] = json!(
            self.input_count
                - self
                    .saved_water
                    .as_ref()
                    .context("Missing progress")?
                    .historical_inputs
        );
        Ok(())
    }
}

pub(super) fn validate_restart(ack: &Value, server_hash: &str) -> Result<()> {
    ensure!(
        ack["checkpoint"] == "after_quest"
            && ack["restart_mode"] == "graceful"
            && ack["same_isolated_database"] == true
            && ack["server_binary_sha256"] == server_hash
            && ack["game_root_identity"]
                == json!({
                    "world_id": WORLD, "artifact_sha256": TO,
                    "descriptor_sha256": "24fec6e152cf5e5a899248bfe302fc12f6f080b71d43f9c0052264f1d9fcbc6e",
                    "assets_manifest_sha256": "95d7afc9d2740299c6e5091b547dbcfbc1b7b2c6f5614eca0b2251adc60abf08"
                }),
        "Saved-water restart changed the exact server/world/b2 delivery"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use clubscape_protocol::{ClientMessage, PROTOCOL_VERSION};
    use prost::Message;
    use std::collections::{BTreeMap, BTreeSet};

    fn selected_source() -> source::Source {
        let content = json!({"regions":{"synthetic":{"cells":[{
            "tile":{"x":3208,"y":3216,"plane":0},"walkable":true,
            "blocked_movement":0,"blocked_sight":0
        }]}}, "spawns":{}, "mechanics":{"object_transforms":{}}});
        source::Source {
            navigation: source::Navigation::from_content(&content).unwrap(),
            content,
            identity: json!({"content_artifact":{"uncompressed_sha256":TO}}),
            oracle: json!({}),
            initial: json!({}),
            tutorial: json!({}),
            activities: json!({}),
            travels: json!({}),
        }
    }

    fn fixture() -> (Value, Value) {
        let operation = "11111111-2222-4333-8444-555555555555";
        let input = game::WorldInput {
            sequence: 498,
            action: Some(game::world_input::Action::Walk(game::Walk {
                destination: Some(game::Tile {
                    x: 3208,
                    y: 3216,
                    plane: 0,
                }),
                running: false,
            })),
            ..Default::default()
        };
        let control = ClientMessage {
            protocol_version: PROTOCOL_VERSION,
            request_id: operation.into(),
            command: Some(client_message::Command::JoinWorld(Default::default())),
        };
        let state = json!({
            "tick":3280,"revision":3794,"next_sequence":499,
            "player":{"actor_id":ACTOR,"region":"region.osrs.12850",
                "tile":{"x":3208,"y":3216,"plane":0},"instance":null,
                "tutorial_stage":"stage.tutorial.mainland",
                "quests":{"quest.cooks_assistant":"stage.cooks.completed"},
                "skills":{"skill.cooking":{"xp_tenths":3000,"base_level":4}},"quest_points":2,
                "inventory":[{"index":0,"stack":{"item":"item.flour.pot","quantity":1}},
                    {"index":1,"stack":{"item":"item.bucket","quantity":1}}]}
        });
        let identity = json!({"content_artifact":{"uncompressed_sha256":FROM}});
        let capsule = json!({
            "synthetic_fixture_only":true, "source_identity":identity,
            "last_observed_state":state, "actor_id":ACTOR,
            "private_authentication_do_not_publish":{"account_id":ACCOUNT,"password":"synthetic-only"},
            "automatic_restore":false,"resume_authorized":false,
            "latest_attempt":{"operation_id":operation,"sequence":498,
                "world_input_protobuf":input.encode_to_vec(),
                "observed_response":{"kind":"acknowledgment_received","duplicate":false,"next_sequence":499}},
            "latest_control_request":{"operation_id":operation,
                "client_message_protobuf":control.encode_to_vec(),
                "observed_http_status":200,"observed_error":null,"private_bearer_token":"synthetic-only"}
        });
        let mut report = json!({
            "synthetic_fixture_only":true, "identity":identity,"last_snapshot":state,
            "status":"blocked","full_journey_passed":false,"checks_passed":377,
            "observation_checks_passed":3,"input_count":603,"trace_records":4547,
            "tutorial_edges_passed":vec![json!({"synthetic_edge":true});70],
            "current_action":"cooks.reward_range_actual_recipe",
            "first_failure":{"action":"cooks.reward_range_actual_recipe"}
        });
        for &segment in evidence::SEGMENTS {
            report["segments"][segment] = json!({"status":
                if matches!(segment,"cooks_reward_and_range"|"after_quest_recovery") {
                    "unchecked"
                } else { "passed" }});
        }
        (capsule, report)
    }

    #[test]
    fn synthetic_boundary_preserves_old_source_and_only_admits_the_exact_remaining_pair() {
        let (capsule, report) = fixture();
        let mut source = selected_source();
        let verified = admitted(&capsule, &report, &source).unwrap().verification;
        assert_eq!(verified["next_sequence"], 499);
        assert_eq!(verified["old_reward_replayed"], false);
        assert_eq!(
            report["identity"]["content_artifact"]["uncompressed_sha256"],
            FROM
        );
        assert!(!verified.to_string().contains("synthetic-only"));
        for wrong in [
            FROM,
            "adb24c14f76181a76e2874e97be3e2743a4c475d3ff89a7720b70ae6f6981bbc",
            "1bc1b8347634b8af0bb3df6a75d4f55f6ac9bf235b8fbb4ea5fdb742cad0a2c4",
        ] {
            source.identity["content_artifact"]["uncompressed_sha256"] = json!(wrong);
            assert!(admitted(&capsule, &report, &source).is_err());
        }
    }

    #[test]
    fn synthetic_frontier_rejects_reacquisition_historical_replay_and_unknown_outcomes() {
        for pointer in [
            "/checks_passed",
            "/input_count",
            "/trace_records",
            "/last_snapshot/next_sequence",
            "/last_snapshot/player/quest_points",
            "/last_snapshot/player/skills/skill.cooking/xp_tenths",
            "/last_snapshot/player/inventory/1/stack/quantity",
        ] {
            let (mut capsule, mut report) = fixture();
            *report.pointer_mut(pointer).unwrap() = json!(0);
            capsule["last_observed_state"] = report["last_snapshot"].clone();
            assert!(
                admitted(&capsule, &report, &selected_source()).is_err(),
                "{pointer}"
            );
        }
        for segment in evidence::SEGMENTS {
            let (capsule, mut report) = fixture();
            report["segments"][segment]["status"] = json!("not_the_original_status");
            assert!(admitted(&capsule, &report, &selected_source()).is_err());
        }
        for response in [
            json!({"kind":"unresolved"}),
            json!({"kind":"acknowledgment_received","duplicate":true,"next_sequence":499}),
            json!({"kind":"acknowledgment_received","duplicate":false,"next_sequence":500}),
        ] {
            let (mut capsule, report) = fixture();
            capsule["latest_attempt"]["observed_response"] = response;
            assert!(admitted(&capsule, &report, &selected_source()).is_err());
        }
    }

    #[test]
    fn one_synthetic_bucket_conversion_cannot_touch_flour_xp_quests_or_other_owned_fields() {
        let (_, report) = fixture();
        let before = report["last_snapshot"]["player"].clone();
        let mut after = before.clone();
        after["inventory"][1]["stack"]["item"] = json!("item.water.bucket");
        water_conversion(&before, &after).unwrap();
        for pointer in [
            "/inventory/0/stack/quantity",
            "/inventory/1/stack/quantity",
            "/skills/skill.cooking/xp_tenths",
            "/quest_points",
        ] {
            let mut wrong = after.clone();
            *wrong.pointer_mut(pointer).unwrap() = json!(99);
            assert!(water_conversion(&before, &wrong).is_err());
        }
        assert!(water_conversion(&before, &before).is_err());
    }

    #[test]
    fn only_a_new_acknowledged_water_receipt_not_the_old_cook_reward_can_be_replayed() {
        let known = Receipt {
            operation_id: "11111111-2222-4333-8444-555555555555".into(),
            sequence: 501,
            observed_revision: 4000,
            action: game::world_input::Action::UseItem(game::UseItem {
                inventory_slot: 1,
                target: Some(game::use_item::Target::WorldSpawn(SINK.into())),
            }),
        };
        require_new_replay(&known, Some(&known)).unwrap();
        assert!(require_new_replay(&known, None).is_err());
        let mut old = known.clone();
        old.sequence = 461;
        assert!(require_new_replay(&old, Some(&old)).is_err());
        old = known.clone();
        old.operation_id = "aaaaaaaa-bbbb-4ccc-8ddd-eeeeeeeeeeee".into();
        assert!(require_new_replay(&old, Some(&known)).is_err());
        old = known.clone();
        old.action = game::world_input::Action::CancelActivity(Default::default());
        assert!(require_new_replay(&old, Some(&known)).is_err());
    }

    #[test]
    fn lifecycle_entry_never_registers_creates_or_submits_world_inputs() {
        use client_message::Command;
        for command in [
            Command::Hello(Default::default()),
            Command::Login(Default::default()),
            Command::JoinWorld(Default::default()),
            Command::Logout(Default::default()),
            Command::LeaveWorld(Default::default()),
            Command::PollWorld(Default::default()),
        ] {
            assert!(lifecycle_command(&command));
        }
        for command in [
            Command::Register(Default::default()),
            Command::CreateCharacter(Default::default()),
            Command::WorldInput(Default::default()),
        ] {
            assert!(!lifecycle_command(&command));
        }
    }

    #[test]
    fn selected_public_source_is_minimal_b2_and_sink_contact_is_not_the_failed_east_guess() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let source = source::Source::load_saved_water(&root).unwrap();
        assert_eq!(
            source.identity["content_artifact"]["uncompressed_sha256"],
            TO
        );
        assert_eq!(source.content["spawns"][SINK]["interactions"], json!([]));
        let goals = source
            .item_use_goals(SINK, "recipe.water.bucket", &source.navigation)
            .unwrap();
        assert!(!goals.contains(&source::Tile::new(3206, 3215, 0)));
        assert!(goals.contains(&source::Tile::new(3206, 3216, 0)));
        assert!(
            source
                .navigation
                .route(source::Tile::new(3208, 3216, 0), &goals)
                .is_some()
        );
        assert!(
            source.identity["file_sha256"]
                .get("content/m1/manifest.json")
                .is_none()
        );
    }

    #[test]
    fn phase_guard_and_additional_input_budget_apply_before_any_network_io() {
        use clap::Parser;
        let crate::Scenario::Scenario(arguments) = crate::Arguments::try_parse_from([
            "synthetic",
            "scenario",
            "m1_fresh_account",
            "--max-inputs",
            "128",
        ])
        .unwrap()
        .command
        else {
            panic!("synthetic argument fixture");
        };
        let directory = std::path::PathBuf::from(".local/synthetic-saved-water")
            .join(uuid::Uuid::new_v4().to_string());
        let mut runner = super::super::Runner {
            arguments: *arguments,
            source: selected_source(),
            evidence: evidence::Evidence::new(&directory.join("report.json")).unwrap(),
            connection: crate::Connection::new("http://127.0.0.1:1").unwrap(),
            deadline: std::time::Instant::now() + std::time::Duration::from_secs(5),
            snapshot: Default::default(),
            entities: BTreeMap::new(),
            observed_states: BTreeMap::new(),
            events: vec![],
            event_ids: BTreeSet::new(),
            sequence: 499,
            input_count: 603,
            stalled_polls: 0,
            account_id: ACCOUNT.into(),
            login_name: "synthetic".into(),
            password: "synthetic".into(),
            token: String::new(),
            actor_id: ACTOR.into(),
            world_session: String::new(),
            expected_stage: None,
            onboarding_receipt: None,
            reward_receipt: None,
            private_attempt: None,
            private_control: None,
            resume: None,
            historical_observation: None,
            saved_water: Some(Progress {
                phase: Phase::Entry,
                historical_inputs: 603,
                receipt: None,
                server_hash: "f".repeat(64),
            }),
        };
        let receipt = Receipt {
            operation_id: "synthetic".into(),
            sequence: 499,
            observed_revision: 4000,
            action: game::world_input::Action::Walk(Default::default()),
        };
        assert!(runner.input_budget_available());
        assert!(validate_input(&runner, &receipt, false).is_err());
        runner.saved_water.as_mut().unwrap().phase = Phase::Water;
        validate_input(&runner, &receipt, false).unwrap();
        let mut forbidden = receipt.clone();
        forbidden.action = game::world_input::Action::DialogueChoice(Default::default());
        assert!(validate_input(&runner, &forbidden, false).is_err());
        runner.saved_water.as_mut().unwrap().phase = Phase::Recovery;
        assert!(validate_input(&runner, &receipt, false).is_err());
        runner.input_count = 730;
        assert!(runner.input_budget_available());
        runner.input_count = 731;
        assert!(!runner.input_budget_available());
        drop(runner);
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn synthetic_full_reader_accepts_migration_lineage_without_relaxing_any_old_resume_mode() {
        use std::{fs, io::Write, os::unix::fs::OpenOptionsExt};
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let selected = source::Source::load_saved_water(&root).unwrap();
        let (mut capsule, mut report) = fixture();
        capsule["schema_version"] = json!(1);
        capsule["kind"] = json!("private_m1_client_checkpoint");
        report["tutorial_edges_passed"] = json!(
            super::super::plan::TUTORIAL_STAGES.windows(2).map(|pair| {
                let from = format!("stage.tutorial.{}", pair[0]);
                let to = format!("stage.tutorial.{}", pair[1]);
                json!({"from":from,"to":to,"source_transition":selected.tutorial_edge(&from,&to).unwrap()["id"]})
            }).collect::<Vec<_>>()
        );
        let directory = std::path::PathBuf::from(".local/journey-runs")
            .join(&uuid::Uuid::new_v4().simple().to_string()[..16]);
        let control = directory.join("control");
        evidence::private_directory(&control).unwrap();
        for (name, value) in [
            ("resume-client-checkpoint.json", &capsule),
            ("resume-report.json", &report),
        ] {
            let mut file = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(control.join(name))
                .unwrap();
            file.write_all(&serde_json::to_vec(value).unwrap()).unwrap();
        }
        let trace = (1..=4547)
            .map(|index| {
                format!("{{\"index\":{index},\"kind\":\"synthetic_history\",\"data\":{{}}}}\n")
            })
            .collect::<String>();
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(control.join("resume-trace.jsonl"))
            .unwrap();
        file.write_all(trace.as_bytes()).unwrap();
        drop(file);
        let path = control.join("resume-client-checkpoint.json");
        let resumed = checkpoint::Resume::load_mode(
            &path,
            &selected,
            checkpoint::ResumeMode::ContinueSavedWater,
        )
        .unwrap();
        assert!(resumed.saved_water.is_some());
        for mode in [
            checkpoint::ResumeMode::Standard,
            checkpoint::ResumeMode::ContinueCook,
            checkpoint::ResumeMode::ContinueMainland,
            checkpoint::ResumeMode::ObserveDying,
        ] {
            assert!(checkpoint::Resume::load_mode(&path, &selected, mode).is_err());
        }
        let mut continued = evidence::Evidence::resume(
            &directory.join("continued.json"),
            &resumed.report,
            &resumed.trace,
        )
        .unwrap();
        continued
            .append("synthetic_b2_continuation", json!({"source":TO}))
            .unwrap();
        continued.flush().unwrap();
        assert!(
            fs::read(directory.join("continued.trace.jsonl"))
                .unwrap()
                .starts_with(trace.as_bytes())
        );
        for segment in evidence::SEGMENTS {
            assert_eq!(
                continued.report["segments"][segment],
                report["segments"][segment]
            );
        }
        assert_eq!(continued.report["identity"], report["identity"]);
        drop(continued);
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn postquest_restart_requires_the_exact_b2_world_descriptor_and_server_hash() {
        let hash = "f".repeat(64);
        let ack = json!({
            "checkpoint":"after_quest","restart_mode":"graceful","same_isolated_database":true,
            "server_binary_sha256":hash,
            "game_root_identity":{"world_id":WORLD,"artifact_sha256":TO,
                "descriptor_sha256":"24fec6e152cf5e5a899248bfe302fc12f6f080b71d43f9c0052264f1d9fcbc6e",
                "assets_manifest_sha256":"95d7afc9d2740299c6e5091b547dbcfbc1b7b2c6f5614eca0b2251adc60abf08"}
        });
        validate_restart(&ack, &hash).unwrap();
        for pointer in [
            "/checkpoint",
            "/server_binary_sha256",
            "/same_isolated_database",
            "/game_root_identity/world_id",
            "/game_root_identity/artifact_sha256",
            "/game_root_identity/descriptor_sha256",
            "/game_root_identity/assets_manifest_sha256",
        ] {
            let mut changed = ack.clone();
            *changed.pointer_mut(pointer).unwrap() = json!("not-the-admitted-target");
            assert!(validate_restart(&changed, &hash).is_err());
        }
    }
}

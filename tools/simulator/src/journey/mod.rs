mod checkpoint;
mod cook;
mod evidence;
#[cfg(test)]
mod history_tests;
mod mainland;
mod observation;
mod plan;
mod recovery;
mod saved_water;
mod source;
mod ui;

use std::{
    collections::{BTreeMap, BTreeSet},
    path::PathBuf,
    time::{Duration, Instant},
};

use anyhow::{Context, Result, bail, ensure};
use clubscape_protocol::{
    CurrentAccount, ErrorCode, Hello, Register, client_message::Command, game,
    server_message::Result as Outcome,
};
use game::world_input::Action;
use serde_json::{Value, json};
use uuid::Uuid;

use crate::Connection;
use evidence::{Evidence, action_json, public_snapshot};
use source::{Source, Tile};

const SOURCE_TICK: Duration = Duration::from_millis(600);

#[derive(clap::Args)]
pub struct Arguments {
    #[arg(value_parser = ["m1_fresh_account"])]
    name: String,
    #[arg(long, default_value = "http://127.0.0.1:4010")]
    url: String,
    #[arg(long, default_value = ".local/evidence/m1-fresh-account.json")]
    report: PathBuf,
    #[arg(long, default_value = ".")]
    source_root: PathBuf,
    /// Owned orchestrator handshake directory. Absence blocks the required restart checkpoint.
    #[arg(long)]
    recovery_control_dir: Option<PathBuf>,
    /// Private owner-only recovery capsule on a blocker; never a resume or state-setting input.
    #[arg(long, requires = "recovery_control_dir")]
    private_checkpoint_file: Option<PathBuf>,
    /// Explicit owner-verified restoration at a supported source continuation boundary.
    #[arg(long, requires = "recovery_control_dir")]
    resume_client_checkpoint: Option<PathBuf>,
    /// Bounded observation only: no player WorldInput or full journey continuation.
    #[arg(long, requires = "resume_client_checkpoint")]
    observe_dying: bool,
    /// Explicit continuation of the accepted same-account Office observation.
    #[arg(
        long,
        requires = "resume_client_checkpoint",
        conflicts_with = "observe_dying"
    )]
    continue_mainland: bool,
    /// Continue only the already-accepted Cook acquisition from its protected checkpoint.
    #[arg(
        long,
        requires = "resume_client_checkpoint",
        conflicts_with_all = ["observe_dying", "continue_mainland"]
    )]
    continue_cook: bool,
    /// Exact migration-aware saved52 remainder; requires a separately admitted reservation.
    #[arg(long, requires_all = ["resume_client_checkpoint", "private_checkpoint_file",
        "source_manifest", "saved_water_admission_revision", "saved_water_admission_sha256",
        "saved_water_executor"], conflicts_with_all = ["observe_dying", "continue_mainland", "continue_cook"])]
    continue_saved_water: bool,
    #[arg(long, requires = "continue_saved_water")]
    source_manifest: Option<PathBuf>,
    #[arg(long, requires = "continue_saved_water")]
    saved_water_admission_revision: Option<String>,
    #[arg(long, requires = "continue_saved_water")]
    saved_water_admission_sha256: Option<String>,
    #[arg(long, requires = "continue_saved_water")]
    saved_water_executor: Option<String>,
    /// Decode/verify an observation resume control without any network operation.
    #[arg(long, requires = "resume_client_checkpoint")]
    validate_resume_only: bool,
    #[arg(long)]
    expected_server_build: Option<String>,
    #[arg(long, default_value_t = 5400, value_parser = clap::value_parser!(u64).range(30..=7200))]
    max_seconds: u64,
    #[arg(long, default_value_t = 6000, value_parser = clap::value_parser!(u64).range(1..=10000))]
    max_inputs: u64,
}

#[derive(Clone)]
struct Receipt {
    operation_id: String,
    sequence: u64,
    observed_revision: u64,
    action: Action,
}

#[derive(Debug)]
struct RpcRejected {
    context: &'static str,
    status: u16,
    code: i32,
    error_id: String,
    message: String,
}

impl std::fmt::Display for RpcRejected {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "Real {} rejected: HTTP {}, code={}, error_id={}, reason={}",
            self.context, self.status, self.code, self.error_id, self.message
        )
    }
}

impl std::error::Error for RpcRejected {}

struct Runner {
    arguments: Arguments,
    source: Source,
    evidence: Evidence,
    connection: Connection,
    deadline: Instant,
    snapshot: game::WorldSnapshot,
    entities: BTreeMap<String, game::Entity>,
    observed_states: BTreeMap<String, String>,
    events: Vec<game::Event>,
    event_ids: BTreeSet<String>,
    sequence: u64,
    input_count: u64,
    stalled_polls: u32,
    account_id: String,
    login_name: String,
    password: String,
    token: String,
    actor_id: String,
    world_session: String,
    expected_stage: Option<(String, String)>,
    onboarding_receipt: Option<Receipt>,
    reward_receipt: Option<Receipt>,
    private_attempt: Option<checkpoint::Attempt>,
    private_control: Option<checkpoint::ControlAttempt>,
    resume: Option<checkpoint::Resume>,
    historical_observation: Option<Value>,
    saved_water: Option<saved_water::Progress>,
}

pub async fn run(arguments: Arguments) -> Result<()> {
    let water_gate = arguments
        .continue_saved_water
        .then(|| saved_water::claim_gate(&arguments))
        .transpose()?;
    let loaded = if arguments.continue_saved_water {
        Source::load_saved_water(&arguments.source_root)
    } else {
        Source::load(&arguments.source_root)
    };
    let source = match loaded {
        Ok(source) => source,
        Err(error) => {
            let mut evidence = Evidence::new(&arguments.report)?;
            evidence.report["status"] = json!("blocked");
            evidence.report["first_failure"] =
                json!({"phase":"source_inputs", "reason":format!("{error:#}")});
            evidence.flush()?;
            return Err(error);
        }
    };
    let resume = arguments
        .resume_client_checkpoint
        .as_ref()
        .map(|path| {
            let mode = if arguments.continue_saved_water {
                checkpoint::ResumeMode::ContinueSavedWater
            } else if arguments.continue_cook {
                ensure!(
                    arguments.max_seconds <= 5400,
                    "Cook continuation exceeds its source budget"
                );
                checkpoint::ResumeMode::ContinueCook
            } else if arguments.continue_mainland {
                checkpoint::ResumeMode::ContinueMainland
            } else if arguments.observe_dying {
                checkpoint::ResumeMode::ObserveDying
            } else {
                checkpoint::ResumeMode::Standard
            };
            checkpoint::Resume::load_mode(path, &source, mode)
        })
        .transpose()?;
    if arguments.validate_resume_only {
        let saved = resume
            .as_ref()
            .context("Observation validation requires a checkpoint")?;
        ensure!(
            arguments.observe_dying && saved.observation_boundary.is_some()
                || arguments.continue_mainland && saved.mainland_boundary.is_some()
                || arguments.continue_cook && saved.cook.is_some()
                || arguments.continue_saved_water && saved.saved_water.is_some(),
            "Not an authorized observation boundary"
        );
        checkpoint::Attempt::from_saved(&saved.capsule["latest_attempt"])?;
        println!(
            "{}",
            json!({"status": "validated", "observation_boundary": saved.observation_boundary,
                "mainland_boundary": saved.mainland_boundary,
                "cook_boundary": saved.cook.as_ref().map(|cook| &cook.verification),
                "saved_water_boundary": saved.saved_water.as_ref().map(|water| &water.verification),
            "network_operations": 0, "world_inputs": 0, "private_payloads_published": false})
        );
        return Ok(());
    }
    let mut evidence = if let Some(resume) = &resume {
        Evidence::resume(&arguments.report, &resume.report, &resume.trace)?
    } else {
        Evidence::new(&arguments.report)?
    };
    evidence.report["limits"] = json!({
        "max_seconds": arguments.max_seconds, "max_inputs": arguments.max_inputs,
        "poll_interval_ms": 600, "stalled_clock_polls": 20,
        "rng_override": false, "gameplay_sql": false
    });
    evidence.report["observation_only"] = json!(arguments.observe_dying);
    evidence.report["mainland_continuation"] = json!(arguments.continue_mainland);
    evidence.report["cook_continuation"] = json!(arguments.continue_cook);
    if arguments.continue_saved_water {
        let saved = resume.as_ref().context("Saved-water history missing")?;
        evidence.report["saved_water_continuation"] = json!(true);
        evidence.report["saved_water_history"] = json!({
            "source_identity": saved.report["identity"],
            "checks_passed": saved.report["checks_passed"],
            "observation_checks_passed": saved.report["observation_checks_passed"],
            "input_count": saved.report["input_count"],
            "trace_records": saved.report["trace_records"],
            "completed_segments": saved.report["segments"],
            "previous_resume_lineage": saved.report["private_checkpoint_resume"],
            "original_report_sha256": source::hash(&std::fs::read(
                arguments.resume_client_checkpoint.as_ref().context("Missing capsule")?
                    .with_file_name("resume-report.json"))?),
            "original_capsule_sha256": source::hash(&std::fs::read(
                arguments.resume_client_checkpoint.as_ref().context("Missing capsule")?)?),
            "historical_work_performed_on_target": false
        });
        evidence.report["source_transition"] = json!({
            "from_artifact": saved_water::FROM, "to_artifact": saved_water::TO,
            "next_sequence": saved_water::NEXT_SEQUENCE,
            "execution_admission_sha256": arguments.saved_water_admission_sha256,
            "multi_artifact_journey": true, "original_checkpoint_rewritten": false
        });
    }
    evidence.report["identity"] = source.identity.clone();
    let connection = match Connection::new(&arguments.url) {
        Ok(connection) => connection,
        Err(error) => {
            evidence.report["status"] = json!("blocked");
            evidence.report["first_failure"] =
                json!({"phase":"loopback_origin", "reason":format!("{error:#}")});
            evidence.flush()?;
            return Err(error);
        }
    };
    let deadline = Instant::now() + Duration::from_secs(arguments.max_seconds);
    let event_ids = resume
        .as_ref()
        .map(|value| value.event_ids.clone())
        .unwrap_or_default();
    let input_count = resume
        .as_ref()
        .map(|value| {
            value.report["input_count"]
                .as_u64()
                .context("Missing historical input count")
        })
        .transpose()?
        .unwrap_or(0);
    let mut runner = Runner {
        arguments,
        source,
        evidence,
        connection,
        deadline,
        snapshot: game::WorldSnapshot::default(),
        entities: BTreeMap::new(),
        observed_states: BTreeMap::new(),
        events: Vec::new(),
        event_ids,
        sequence: 0,
        input_count,
        stalled_polls: 0,
        account_id: String::new(),
        login_name: format!("m1_{}", &Uuid::new_v4().simple().to_string()[..14]),
        password: format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple()),
        token: String::new(),
        actor_id: String::new(),
        world_session: String::new(),
        expected_stage: None,
        onboarding_receipt: None,
        reward_receipt: None,
        private_attempt: None,
        private_control: None,
        resume,
        historical_observation: None,
        saved_water: water_gate
            .map(|gate| -> Result<saved_water::Progress> {
                Ok(saved_water::Progress {
                    phase: saved_water::Phase::Entry,
                    historical_inputs: input_count,
                    receipt: None,
                    server_hash: gate["server_sha256"]
                        .as_str()
                        .context("Admitted restart server hash missing")?
                        .to_owned(),
                })
            })
            .transpose()?,
    };
    let result = runner.execute().await;
    runner.evidence.report["input_count"] = json!(runner.input_count);
    runner.evidence.report["unique_events_observed"] = json!(runner.event_ids.len());
    runner.evidence.report["last_sequence"] = json!(runner.sequence.saturating_sub(1));
    if runner.snapshot.player.is_some() {
        runner.evidence.report["last_snapshot"] = public_snapshot(&runner.snapshot)?;
    }
    if runner.arguments.observe_dying || runner.arguments.continue_saved_water {
        runner.evidence.report["observation_snapshot_origin"] =
            json!(if runner.snapshot.player.is_some() {
                "actual_current_invocation"
            } else {
                "unchanged_historical_public_snapshot_no_new_observation"
            });
    }
    match &result {
        Ok(()) if runner.arguments.observe_dying => {
            runner.evidence.report["status"] = json!("observed");
            runner.evidence.report["full_journey_passed"] = json!(false);
        }
        Ok(()) if runner.arguments.continue_saved_water => {
            runner.evidence.report["status"] = json!("continued");
            runner.evidence.report["saved_water_remainder_passed"] = json!(true);
            runner.evidence.report["full_journey_passed"] = json!(false);
        }
        Ok(()) => {
            ensure!(
                evidence::SEGMENTS
                    .iter()
                    .all(
                        |segment| runner.evidence.report["segments"][segment]["status"] == "passed"
                    ),
                "Runner returned without all required segments"
            );
            runner.evidence.report["status"] = json!("passed");
            runner.evidence.report["full_journey_passed"] = json!(true);
        }
        Err(error) => {
            runner.evidence.report["status"] = json!("blocked");
            runner.evidence.report["first_failure"] = json!({
                "action": runner.evidence.report["current_action"],
                "stage": runner.snapshot.player.as_ref().map(|player| &player.tutorial_stage),
                "sequence": runner.sequence, "tick": runner.snapshot.tick,
                "reason": format!("{error:#}"),
                "unknown_or_unchecked_is_not_success": true
            });
        }
    }
    runner.evidence.flush()?;
    if let Some(path) = &runner.arguments.private_checkpoint_file {
        runner.evidence.report["private_client_checkpoint"] =
            match checkpoint::capture(&runner, path) {
                Ok(status) => status,
                Err(error) => json!({
                    "status": "failed",
                    "reason": format!("{error:#}"),
                    "recoverable_checkpoint": false
                }),
            };
        runner.evidence.flush()?;
    }
    println!(
        "{}",
        json!({
            "scenario": "m1_fresh_account",
            "status": runner.evidence.report["status"],
            "full_journey_passed": runner.evidence.report["full_journey_passed"],
            "checks_passed": runner.evidence.report["checks_passed"],
            "evidence": runner.arguments.report,
            "milestone_accepted": false
        })
    );
    result
}

impl Runner {
    fn bound(&self) -> Result<()> {
        ensure!(
            Instant::now() < self.deadline,
            "Journey wall-clock budget exhausted"
        );
        Ok(())
    }

    fn input_budget_available(&self) -> bool {
        let consumed = self.saved_water.as_ref().map_or(self.input_count, |water| {
            self.input_count.saturating_sub(water.historical_inputs)
        });
        consumed < self.arguments.max_inputs
    }

    fn player(&self) -> Result<&game::Player> {
        self.snapshot
            .player
            .as_ref()
            .context("No authoritative player snapshot")
    }

    fn tile(&self) -> Result<Tile> {
        self.player()?
            .tile
            .as_ref()
            .map(Tile::from)
            .context("No authoritative player position")
    }

    fn count(&self, item: &str) -> Result<u64> {
        Ok(self
            .player()?
            .inventory
            .iter()
            .filter_map(|slot| slot.stack.as_ref())
            .filter(|stack| stack.item == item)
            .map(|stack| u64::from(stack.quantity))
            .sum())
    }

    fn owned(&self, item: &str) -> Result<u64> {
        Ok(self.count(item)?
            + self
                .player()?
                .equipment
                .iter()
                .filter_map(|slot| slot.stack.as_ref())
                .filter(|stack| stack.item == item)
                .map(|stack| u64::from(stack.quantity))
                .sum::<u64>())
    }

    fn slot(&self, item: &str) -> Result<u32> {
        self.player()?
            .inventory
            .iter()
            .find(|slot| slot.stack.as_ref().is_some_and(|stack| stack.item == item))
            .map(|slot| slot.index)
            .with_context(|| format!("No legitimately owned inventory slot for {item}"))
    }

    fn xp(&self, skill: &str) -> Result<u64> {
        self.player()?
            .skills
            .iter()
            .find(|entry| entry.id == skill)
            .map(|entry| entry.xp_tenths)
            .with_context(|| format!("Public player has no skill {skill}"))
    }

    fn quest(&self, quest: &str) -> Result<&str> {
        self.player()?
            .quests
            .iter()
            .find(|entry| entry.id == quest)
            .map(|entry| entry.stage.as_str())
            .with_context(|| format!("Public player has no quest {quest}"))
    }

    fn label(&mut self, name: &str) -> Result<()> {
        self.bound()?;
        self.evidence.report["current_action"] = json!(name);
        self.evidence.flush()
    }

    async fn rpc(&mut self, command: Command, token: bool) -> Result<Outcome> {
        self.bound()?;
        ensure!(
            !self.arguments.observe_dying || observation::allowed(&command),
            "Observation-only mode forbids this RPC"
        );
        ensure!(
            self.saved_water.is_none() || saved_water::lifecycle_command(&command),
            "Saved-water forbids registration, character creation and non-lifecycle RPCs"
        );
        let request_id = Uuid::new_v4().to_string();
        let observed_command = observation::command_name(&command);
        let track = self.arguments.private_checkpoint_file.is_some()
            && (self.arguments.observe_dying
                || !matches!(
                    &command,
                    Command::Hello(_) | Command::CurrentAccount(_) | Command::PollWorld(_)
                ));
        if self.arguments.observe_dying {
            self.evidence.append(
                "observation_rpc_request",
                json!({
                    "request_id": request_id, "command": observed_command, "world_input": false
                }),
            )?;
        }
        if track {
            self.private_control = Some(checkpoint::ControlAttempt::new(
                request_id.clone(),
                command.clone(),
                token.then(|| self.token.clone()),
            ));
        }
        let (status, result) = self
            .connection
            .request_with_id(command, token.then_some(self.token.as_str()), &request_id)
            .await?;
        if self.arguments.observe_dying {
            self.evidence.append("observation_rpc_response", json!({
                "request_id": request_id, "command": observed_command,
                "http_status": status.as_u16(),
                "error_id": match &result { Outcome::Error(error) => Some(&error.error_id), _ => None }
            }))?;
        }
        if track && let Some(attempt) = &mut self.private_control {
            let error = match &result {
                Outcome::Error(error) => Some((error.code, error.error_id.clone())),
                _ => None,
            };
            attempt.received(status.as_u16(), error);
        }
        if let Outcome::Error(error) = &result {
            return Err(RpcRejected {
                context: "RPC",
                status: status.as_u16(),
                code: error.code,
                error_id: error.error_id.clone(),
                message: error.message.clone(),
            }
            .into());
        }
        ensure!(status.is_success(), "RPC failed with HTTP {status}");
        Ok(result)
    }

    async fn hello(&mut self) -> Result<()> {
        self.label("hello.gameplay_readiness")?;
        let Outcome::Hello(hello) = self.rpc(Command::Hello(Hello {}), false).await? else {
            bail!("Hello returned the wrong generated Protobuf result");
        };
        self.evidence.report["server_build_revision"] = json!(hello.build_revision);
        self.evidence.report["server_capabilities"] = json!(hello.capabilities);
        if let Some(expected) = &self.arguments.expected_server_build {
            self.evidence.check(
                "exact_server_build",
                json!(expected),
                json!(hello.build_revision),
            )?;
        }
        ensure!(
            hello.gameplay_available
                && hello
                    .capabilities
                    .iter()
                    .any(|capability| capability == "game.v1"),
            "Real server is not gameplay-ready: {}",
            hello.gameplay_unavailable_reason
        );
        ensure!(
            hello
                .capabilities
                .iter()
                .any(|capability| capability == "game.ui.v1")
                && hello
                    .capabilities
                    .iter()
                    .any(|capability| capability == "game.observer.v1"),
            "Required versioned source UI/action observer capabilities are unavailable"
        );
        self.evidence.check(
            "server_request_budget",
            json!(clubscape_protocol::MAX_REQUEST_BYTES),
            json!(hello.max_request_bytes),
        )?;
        Ok(())
    }

    async fn register_and_join(&mut self) -> Result<()> {
        self.hello().await?;
        self.label("account.register")?;
        let Outcome::Registered(registered) = self
            .rpc(
                Command::Register(Register {
                    login_name: self.login_name.clone(),
                    password: self.password.clone(),
                }),
                false,
            )
            .await?
        else {
            bail!("Registration did not return a real account");
        };
        let account = registered.account.context("Registered account missing")?;
        self.account_id = account.account_id;
        self.evidence.report["synthetic_account_id"] = json!(self.account_id);
        self.label("account.login")?;
        self.login().await?;
        let Outcome::Account(current) = self
            .rpc(Command::CurrentAccount(CurrentAccount {}), true)
            .await?
        else {
            bail!("CurrentAccount returned the wrong result");
        };
        self.evidence.check(
            "fresh_account_no_character",
            json!(false),
            json!(current.character_initialized),
        )?;
        self.evidence.passed("registration_login")?;
        self.label("game.create_source_character")?;
        let Outcome::CharacterCreated(created) = self
            .rpc(
                Command::CreateCharacter(game::CreateCharacter::default()),
                true,
            )
            .await?
        else {
            bail!("Character creation did not return a source character");
        };
        self.evidence.check(
            "created_content_revision",
            json!(self.source.revision()?),
            json!(created.content_revision),
        )?;
        self.actor_id = created.actor_id;
        self.join().await?;
        self.check_initial()?;
        self.evidence.passed("source_initial_character")
    }

    async fn login(&mut self) -> Result<()> {
        let Outcome::LoggedIn(session) = self
            .rpc(
                Command::Login(clubscape_protocol::Login {
                    login_name: self.login_name.clone(),
                    password: self.password.clone(),
                }),
                false,
            )
            .await?
        else {
            bail!("Login did not return a session");
        };
        ensure!(
            session.session_token.len() == 43 && session.expires_at_unix_ms > 0,
            "Server returned an invalid session"
        );
        ensure!(
            session
                .account
                .as_ref()
                .is_some_and(|account| account.account_id == self.account_id),
            "Login changed synthetic account identity"
        );
        self.token = session.session_token;
        Ok(())
    }

    async fn join(&mut self) -> Result<()> {
        let Outcome::WorldJoined(joined) = self
            .rpc(Command::JoinWorld(game::JoinWorld {}), true)
            .await?
        else {
            bail!("JoinWorld did not return an authoritative world snapshot");
        };
        ensure!(
            Uuid::parse_str(&joined.world_session_id).is_ok(),
            "Missing world session identity"
        );
        self.evidence.check(
            "joined_content_revision",
            json!(self.source.revision()?),
            json!(joined.content_revision),
        )?;
        ensure!(
            joined.next_sequence > 0,
            "JoinWorld returned an invalid sequence"
        );
        let snapshot = joined.snapshot.context("JoinWorld snapshot missing")?;
        ensure!(
            snapshot.full_snapshot,
            "JoinWorld must supply a complete entity baseline"
        );
        ensure!(
            snapshot.next_sequence == joined.next_sequence,
            "Join/snapshot sequence mismatch"
        );
        self.world_session = joined.world_session_id;
        self.sequence = joined.next_sequence;
        self.evidence.report["content_manifest_path"] = json!(joined.content_manifest_path);
        self.capture(snapshot, "join")
    }

    fn capture(&mut self, snapshot: game::WorldSnapshot, reason: &str) -> Result<()> {
        ensure!(
            snapshot.ui.as_ref().is_some_and(|ui| ui.version == 1),
            "M1 requires an actual complete UIstate1 projection; absence is not empty successful UI"
        );
        let player = snapshot
            .player
            .as_ref()
            .context("Snapshot omitted the local player")?;
        ensure!(
            player.actor_id == self.actor_id,
            "Snapshot changed authoritative actor identity"
        );
        ensure!(
            snapshot.revision >= self.snapshot.revision && snapshot.tick >= self.snapshot.tick,
            "Server revision/tick went backwards"
        );
        if snapshot.event_history_gap {
            ensure!(
                reason == "action_ack_history_suffix"
                    && Self::history_suffix_covered(self.events.last(), &snapshot.events),
                "Authoritative event history gap: acceptance evidence is incomplete"
            );
            self.evidence.append("input_history_suffix_verified", json!({
                "last_observed_event_id": self.events.last().map(|event| &event.event_id),
                "prior_observed_revision": self.snapshot.revision,
                "acknowledged_revision": snapshot.revision,
                "server_history_floor": snapshot.event_history_floor_revision,
                "wire_gap_preserved": true,
                "proof": "The server returns a chronological suffix, dropping only its oldest events. An exact last-observed event is present, so no later event is omitted by that suffix truncation.",
                "input_replayed": false
            }))?;
        }
        ensure!(
            snapshot.next_sequence > 0,
            "Snapshot omitted authoritative next_sequence"
        );
        if self
            .snapshot
            .player
            .as_ref()
            .is_some_and(|previous| previous.tutorial_stage == "stage.tutorial.mainland")
        {
            ensure!(
                player.tutorial_stage == "stage.tutorial.mainland",
                "A mainland character regressed into the tutorial"
            );
        }
        ensure!(
            player.inventory.len() <= 28,
            "Inventory exceeds source 28 slots"
        );
        let mut indices = BTreeSet::new();
        for slot in &player.inventory {
            ensure!(
                slot.index < 28 && indices.insert(slot.index),
                "Invalid/duplicate inventory slot"
            );
            let stack = slot
                .stack
                .as_ref()
                .context("Inventory slot omitted its stack")?;
            ensure!(
                stack.quantity > 0 && self.source.content["items"].get(&stack.item).is_some(),
                "Unknown/empty item stack from server"
            );
        }
        if let Some((from, to)) = &self.expected_stage {
            ensure!(
                &player.tutorial_stage == from || &player.tutorial_stage == to,
                "Unexpected tutorial stage {} during explicit {from} -> {to} plan; no stages skipped",
                player.tutorial_stage
            );
        }
        if snapshot.full_snapshot {
            self.entities.clear();
        }
        for id in &snapshot.removed_entities {
            self.entities.remove(id);
        }
        for entity in &snapshot.entities {
            self.entities.insert(entity.id.clone(), entity.clone());
        }
        for object in &snapshot.dynamic_objects {
            if let Some(state) = &object.state {
                self.observed_states
                    .insert(object.definition_id.clone(), state.clone());
            }
        }
        let mut fresh = Vec::new();
        for event in &snapshot.events {
            ensure!(
                !event.event_id.is_empty(),
                "Source event lacks a durable event ID"
            );
            ensure!(
                event.actor_id == self.actor_id,
                "Snapshot leaked another actor's routed event"
            );
            if self.event_ids.insert(event.event_id.clone()) {
                fresh.push(evidence::message_json("clubscape.game.v1.Event", event)?);
                self.events.push(event.clone());
            }
        }
        ensure!(
            self.event_ids.len() <= 65536,
            "Event identity budget exhausted"
        );
        if self.events.len() > 2048 {
            self.events.drain(..self.events.len() - 2048);
        }
        self.evidence.append(
            "authoritative_snapshot",
            json!({
                "reason": reason, "state": public_snapshot(&snapshot)?, "new_events": fresh
            }),
        )?;
        self.sequence = snapshot.next_sequence;
        self.snapshot = snapshot;
        Ok(())
    }

    async fn input(&mut self, action: Action) -> Result<Receipt> {
        ensure!(
            !self.arguments.observe_dying,
            "Observation-only mode forbids gameplay and UI input"
        );
        if !matches!(action, Action::Ui(_)) {
            self.continue_source_presentations().await?;
        }
        self.input_raw(action).await
    }

    async fn input_raw(&mut self, action: Action) -> Result<Receipt> {
        let receipt = Receipt {
            operation_id: Uuid::new_v4().to_string(),
            sequence: self.sequence,
            observed_revision: self.snapshot.character_revision,
            action,
        };
        self.submit(&receipt, false).await?;
        Ok(receipt)
    }

    async fn submit(&mut self, receipt: &Receipt, duplicate: bool) -> Result<()> {
        self.bound()?;
        ensure!(
            !self.arguments.observe_dying,
            "Observation-only mode forbids all WorldInput, including replay"
        );
        ensure!(
            self.input_budget_available(),
            "Journey input budget exhausted"
        );
        if self.saved_water.is_some() {
            saved_water::validate_input(self, receipt, duplicate)?;
        }
        let before_sequence = self.sequence;
        let input = game::WorldInput {
            world_session_id: self.world_session.clone(),
            sequence: receipt.sequence,
            expected_character_revision: Some(receipt.observed_revision),
            action: Some(receipt.action.clone()),
        };
        self.evidence.append(
            "input",
            json!({
                "operation_id": receipt.operation_id, "sequence": receipt.sequence,
                "observed_character_revision": receipt.observed_revision,
                "retry_of_same_operation": duplicate, "input": action_json(&receipt.action)?,
                "before": public_snapshot(&self.snapshot)?
            }),
        )?;
        self.input_count += 1;
        self.private_attempt = self
            .arguments
            .private_checkpoint_file
            .as_ref()
            .map(|_| checkpoint::Attempt::new(receipt.operation_id.clone(), input.clone()));
        let (status, result) = self
            .connection
            .request_with_id(
                Command::WorldInput(input),
                Some(&self.token),
                &receipt.operation_id,
            )
            .await?;
        match result {
            Outcome::ActionResult(result) if status.is_success() => {
                ensure!(
                    result.operation_id == receipt.operation_id
                        && result.sequence == receipt.sequence,
                    "Action result changed operation/sequence identity"
                );
                ensure!(
                    result.duplicate == duplicate,
                    "Server duplicate status does not match the submitted operation history"
                );
                if let Some(attempt) = &mut self.private_attempt {
                    attempt.acknowledged(
                        result.duplicate,
                        result
                            .snapshot
                            .as_ref()
                            .map(|snapshot| snapshot.next_sequence),
                    );
                }
                let snapshot = result.snapshot.context("Action result snapshot missing")?;
                if snapshot.event_history_gap {
                    if Self::history_suffix_covered(self.events.last(), &snapshot.events) {
                        self.capture(snapshot, "action_ack_history_suffix")?;
                    } else {
                        self.reconcile_acknowledged_history(snapshot, receipt)
                            .await?;
                    }
                } else {
                    self.capture(snapshot, "action_ack")?;
                }
                let expected = if duplicate {
                    before_sequence
                } else {
                    receipt.sequence + 1
                };
                ensure!(
                    self.sequence == expected,
                    "Acknowledgment consumed an incorrect number of sequences"
                );
            }
            Outcome::Error(error) => {
                if let Some(attempt) = &mut self.private_attempt {
                    attempt.rejected(status.as_u16(), error.code, error.error_id.clone());
                }
                self.evidence.append("server_rejection", json!({
                    "operation_id": receipt.operation_id, "sequence": receipt.sequence,
                    "http_status": status.as_u16(), "code": error.code, "error_id": error.error_id,
                    "reason": error.message, "retry_after_seconds": error.retry_after_seconds
                }))?;
                if error.code == ErrorCode::Conflict as i32
                    && let Action::ShopBuy(buy) = &receipt.action
                {
                    self.refresh_rejected_shop(buy.expected_item.as_deref(), "shop_buy_conflict")
                        .await?;
                }
                return Err(RpcRejected {
                    context: "game action",
                    status: status.as_u16(),
                    code: error.code,
                    error_id: error.error_id,
                    message: error.message,
                }
                .into());
            }
            _ => bail!("WorldInput returned an unexpected Protobuf result/status {status}"),
        }
        Ok(())
    }

    async fn reconcile_acknowledged_history(
        &mut self,
        acknowledged: game::WorldSnapshot,
        receipt: &Receipt,
    ) -> Result<()> {
        let observed_revision = self.snapshot.revision;
        self.evidence.append("acknowledged_input_history_gap", json!({
            "operation_id": receipt.operation_id,
            "sequence": receipt.sequence,
            "last_continuous_revision": observed_revision,
            "acknowledged_state": public_snapshot(&acknowledged)?,
            "server_event_floor": acknowledged.event_history_floor_revision,
            "policy": "Successful input already acknowledged. Requery actual observed cursor read-only; do not replay the input or claim omitted events were observed."
        }))?;
        self.bound()?;
        tokio::time::sleep(SOURCE_TICK).await;
        let response = self
            .rpc(
                Command::PollWorld(game::PollWorld {
                    world_session_id: self.world_session.clone(),
                    after_revision: observed_revision,
                    quote: None,
                }),
                true,
            )
            .await?;
        let Outcome::WorldSnapshot(recovered) = response else {
            bail!("History reconciliation poll returned no authoritative snapshot");
        };
        Self::validate_history_reconciliation(observed_revision, &acknowledged, &recovered)?;
        self.evidence.append(
            "input_history_reconciled_read_only",
            json!({
                "operation_id": receipt.operation_id,
                "sequence": receipt.sequence,
                "polled_after_revision": observed_revision,
                "acknowledged_revision": acknowledged.revision,
                "continuous_revision": recovered.revision,
                "continuous_server_floor": recovered.event_history_floor_revision,
                "input_replayed": false
            }),
        )?;
        self.capture(recovered, "action_ack_continuous_cursor")
    }

    async fn poll(&mut self) -> Result<()> {
        self.bound()?;
        tokio::time::sleep(SOURCE_TICK).await;
        let result = self
            .rpc(
                Command::PollWorld(game::PollWorld {
                    world_session_id: self.world_session.clone(),
                    after_revision: self.snapshot.revision,
                    quote: None,
                }),
                true,
            )
            .await?;
        let Outcome::WorldSnapshot(snapshot) = result else {
            bail!("PollWorld returned the wrong generated Protobuf result");
        };
        ensure!(
            snapshot.next_sequence == self.sequence,
            "A poll consumed a gameplay sequence"
        );
        self.stalled_polls = if snapshot.tick == self.snapshot.tick {
            self.stalled_polls + 1
        } else {
            0
        };
        ensure!(
            self.stalled_polls < 20,
            "Authoritative 600ms clock stalled for 20 polls"
        );
        self.capture(snapshot, "poll")
    }

    fn validate_history_reconciliation(
        observed_revision: u64,
        acknowledged: &game::WorldSnapshot,
        recovered: &game::WorldSnapshot,
    ) -> Result<()> {
        ensure!(
            !recovered.event_history_gap
                && recovered.event_history_floor_revision <= observed_revision,
            "The actual observed cursor has an event-history gap; acceptance evidence is incomplete"
        );
        ensure!(
            recovered.revision >= acknowledged.revision
                && acknowledged.revision >= observed_revision
                && recovered.tick >= acknowledged.tick,
            "Read-only history reconciliation moved behind the acknowledged input"
        );
        ensure!(
            recovered.next_sequence == acknowledged.next_sequence && recovered.next_sequence > 0,
            "History reconciliation changed the acknowledged input sequence"
        );
        Ok(())
    }

    fn history_suffix_covered(
        last_observed: Option<&game::Event>,
        received: &[game::Event],
    ) -> bool {
        last_observed.is_some_and(|last| {
            !last.event_id.is_empty()
                && received.iter().any(|event| event == last)
                && received
                    .iter()
                    .filter(|event| event.event_id == last.event_id)
                    .count()
                    == 1
        })
    }

    async fn quote(&mut self, request: game::quote_request::Request) -> Result<game::Quote> {
        self.bound()?;
        tokio::time::sleep(SOURCE_TICK).await;
        let sequence = self.sequence;
        let before = evidence::stable_player(self.player()?);
        let expected_item = match &request {
            game::quote_request::Request::ShopBuy(buy) => buy.expected_item.clone(),
            _ => None,
        };
        let request = game::QuoteRequest {
            request: Some(request),
        };
        self.evidence.append(
            "quote_request",
            json!({
                "next_sequence": sequence,
                "request": evidence::message_json("clubscape.game.v1.QuoteRequest", &request)?
            }),
        )?;
        let result = self
            .rpc(
                Command::PollWorld(game::PollWorld {
                    world_session_id: self.world_session.clone(),
                    after_revision: self.snapshot.revision,
                    quote: Some(request),
                }),
                true,
            )
            .await;
        let result = match result {
            Ok(result) => result,
            Err(error) => {
                if expected_item.is_some() {
                    self.refresh_rejected_shop(expected_item.as_deref(), "shop_quote_rejected")
                        .await?;
                }
                return Err(error);
            }
        };
        let Outcome::WorldSnapshot(snapshot) = result else {
            bail!("Quote PollWorld returned the wrong generated Protobuf result");
        };
        ensure!(
            snapshot.next_sequence == sequence,
            "A read-only quote consumed a gameplay sequence"
        );
        let quote = snapshot
            .quote
            .clone()
            .context("Actual guarded quote is missing")?;
        self.capture(snapshot, "guarded_quote")?;
        self.evidence.check(
            "quote_preserves_gameplay_state",
            before,
            evidence::stable_player(self.player()?),
        )?;
        Ok(quote)
    }

    async fn refresh_rejected_shop(
        &mut self,
        expected_item: Option<&str>,
        context: &str,
    ) -> Result<()> {
        let sequence = self.sequence;
        let refresh_error = self.poll().await.err().map(|error| format!("{error:#}"));
        self.evidence.append("shop_selection_requires_new_choice", json!({
            "context": context,
            "original_expected_item": expected_item,
            "next_sequence_before_refresh": sequence,
            "next_sequence_after_refresh": self.sequence,
            "view_refresh_error": refresh_error,
            "current_shop": self.snapshot.shop.as_ref().map(|shop|
                evidence::message_json_with_defaults("clubscape.game.v1.ShopView", shop)).transpose()?,
            "retry_submitted": false,
            "policy": "Refresh is read-only. The original intent/identity is retained; no different item or row is chosen automatically."
        }))?;
        ensure!(
            sequence == self.sequence,
            "Rejected shop operation consumed a sequence"
        );
        Ok(())
    }

    async fn wait_for(
        &mut self,
        label: &str,
        ticks: u64,
        predicate: impl Fn(&Self) -> Result<bool>,
    ) -> Result<()> {
        ensure!((1..=1800).contains(&ticks), "Invalid bounded action wait");
        let start = self.snapshot.tick;
        let wall_deadline = Instant::now() + SOURCE_TICK * ticks as u32 + Duration::from_secs(20);
        let mut stalled = 0;
        while !predicate(self)? {
            ensure!(
                self.snapshot.tick.saturating_sub(start) < ticks && Instant::now() < wall_deadline,
                "Bounded wait {label} exhausted {ticks} source ticks; stage={}, activity={}, position={:?}",
                self.player()?.tutorial_stage,
                self.player()?.activity,
                self.tile()?
            );
            let before = self.snapshot.tick;
            self.poll().await?;
            stalled = if self.snapshot.tick == before {
                stalled + 1
            } else {
                0
            };
            ensure!(
                stalled < 20,
                "Source 600ms clock stalled during {label}; no synthetic ticks applied"
            );
        }
        self.evidence.append(
            "bounded_wait_completed",
            json!({
                "label": label, "observed_source_ticks": self.snapshot.tick - start,
                "budget_ticks": ticks
            }),
        )
    }

    async fn cancel(&mut self) -> Result<()> {
        if self.player()?.activity != "idle" {
            self.input(Action::CancelActivity(game::Empty {})).await?;
            self.wait_for("cancel_activity", 10, |runner| {
                Ok(runner.player()?.activity == "idle")
            })
            .await?;
        }
        Ok(())
    }

    fn check_initial(&mut self) -> Result<()> {
        let player = self.player()?.clone();
        self.evidence.check(
            "source_initial_inventory_empty",
            json!(0),
            json!(player.inventory.len()),
        )?;
        self.evidence.check(
            "source_initial_equipment_empty",
            json!(0),
            json!(player.equipment.len()),
        )?;
        self.evidence.check(
            "source_initial_stage",
            self.source.initial["tutorial"]["stage_ref"].clone(),
            json!(player.tutorial_stage),
        )?;
        self.evidence.check(
            "source_initial_position_inference",
            self.source.oracle["tutorial"]["initial_position"].clone(),
            json!(self.tile()?),
        )?;
        self.evidence.check(
            "source_initial_appearance_unconfirmed",
            json!(false),
            json!(player.appearance_confirmed),
        )?;
        self.evidence.check(
            "source_initial_experience_unselected",
            Value::Null,
            json!(player.experience),
        )?;
        for (name, actual) in [
            ("initial_hitpoints", player.hitpoints),
            ("initial_prayer_points", player.prayer_points),
            ("initial_run_energy", player.run_energy),
            ("initial_quest_points", player.quest_points),
        ] {
            self.evidence.check(
                name,
                self.source.oracle["tutorial"][name].clone(),
                json!(actual),
            )?;
        }
        self.evidence.check(
            "source_initial_skill_count",
            json!(24),
            json!(player.skills.len()),
        )?;
        for skill in self.source.initial["skills"]
            .as_array()
            .context("Missing independent initial skills")?
        {
            let id = skill["id"]
                .as_str()
                .context("Source initial skill lacks ID")?;
            let actual = player
                .skills
                .iter()
                .find(|entry| entry.id == id)
                .context("Missing initial source skill")?;
            self.evidence.check(
                id,
                json!({
                    "xp_tenths": skill["xp_tenths"], "base_level": skill["base_level"],
                    "current_level": skill["current_level"]
                }),
                json!({
                    "xp_tenths": actual.xp_tenths, "base_level": actual.base_level,
                    "current_level": actual.current_level
                }),
            )?;
        }
        for (id, stage) in [
            ("quest.cooks_assistant", "stage.cooks.not_started"),
            (
                "quest.learning_the_ropes",
                "stage.learning_the_ropes.not_started",
            ),
        ] {
            self.evidence
                .check(id, json!(stage), json!(self.quest(id)?))?;
        }
        self.evidence.report["initial_source_state"] = public_snapshot(&self.snapshot)?;
        Ok(())
    }

    async fn execute(&mut self) -> Result<()> {
        if self.arguments.continue_saved_water {
            let saved = self
                .resume
                .take()
                .context("Saved-water requires its exact original capsule")?;
            return self.continue_saved_water(saved).await;
        }
        plan::validate_source_path(&self.source)?;
        if self.arguments.observe_dying {
            let saved = self
                .resume
                .take()
                .context("Observation requires an explicit restored checkpoint")?;
            return self.observe_dying(saved).await;
        }
        let after_goblin_kill = self
            .resume
            .as_ref()
            .is_some_and(|resume| resume.after_goblin_kill);
        let mut existing_death = None;
        let mut accepted_cook = None;
        if let Some(resume) = self.resume.take() {
            if self.arguments.continue_mainland {
                self.evidence.report["mainland_continuation_boundary"] = resume
                    .mainland_boundary
                    .clone()
                    .context("Missing authorized mainland boundary")?;
            }
            if self.arguments.continue_cook {
                self.evidence.report["cook_continuation_boundary"] = resume
                    .cook
                    .as_ref()
                    .context("Missing authorized Cook boundary")?
                    .verification
                    .clone();
            }
            self.account_id = resume.string("/private_authentication_do_not_publish/account_id")?;
            self.login_name = resume.string("/private_authentication_do_not_publish/login_name")?;
            self.password = resume.string("/private_authentication_do_not_publish/password")?;
            self.actor_id = resume.string("/actor_id")?;
            self.onboarding_receipt = resume.receipt("onboarding")?;
            self.reward_receipt = resume.receipt("reward")?;
            self.hello().await?;
            self.login().await?;
            self.join().await?;
            self.evidence.check(
                "restored_original_actor",
                json!(self.actor_id),
                json!(self.player()?.actor_id),
            )?;
            self.evidence.check(
                "restored_original_next_sequence",
                resume.capsule["last_observed_state"]["next_sequence"].clone(),
                json!(self.sequence),
            )?;
            let actual = evidence::stable_player(self.player()?);
            let mut expected = serde_json::Map::new();
            for key in actual.as_object().context("Invalid stable player")?.keys() {
                expected.insert(
                    key.clone(),
                    resume.capsule["last_observed_state"]["player"]
                        .get(key)
                        .with_context(|| format!("Missing historical player field {key}"))?
                        .clone(),
                );
            }
            self.evidence.check(
                "restored_acknowledged_player_state",
                Value::Object(expected),
                actual,
            )?;
            if self.arguments.continue_mainland {
                existing_death = Some(
                    resume
                        .existing_death
                        .context("Missing original death baseline")?,
                );
            } else if self.arguments.continue_cook {
                accepted_cook = Some(
                    resume
                        .cook
                        .context("Missing accepted Cook baseline")?
                        .baseline,
                );
            } else {
                self.input(Action::CloseInterface(game::Empty {})).await?;
                let receipt = self
                    .onboarding_receipt
                    .clone()
                    .context("Missing original source grant receipt")?;
                self.verify_duplicate(&receipt, "restored_original_grant_deduplicated")
                    .await?;
            }
        } else {
            self.register_and_join().await?;
        }
        if let Some(baseline) = accepted_cook {
            self.continue_accepted_cook(baseline).await?;
        } else {
            if let Some(death) = existing_death {
                self.continue_existing_death(death).await?;
            } else if after_goblin_kill {
                self.resume_goblin_loot().await?;
            } else {
                self.tutorial().await?;
                self.lumbridge().await?;
            }
            self.cooks_assistant().await?;
        }
        let receipt = self
            .reward_receipt
            .clone()
            .context("Missing acknowledged Cook reward receipt")?;
        self.recovery_checkpoint("after_quest", &receipt).await?;
        self.logout().await?;
        Ok(())
    }
}

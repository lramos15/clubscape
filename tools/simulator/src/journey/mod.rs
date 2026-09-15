mod evidence;
mod plan;
mod recovery;
mod source;

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
}

pub async fn run(arguments: Arguments) -> Result<()> {
    let mut evidence = Evidence::new(&arguments.report)?;
    evidence.report["limits"] = json!({
        "max_seconds": arguments.max_seconds, "max_inputs": arguments.max_inputs,
        "poll_interval_ms": 600, "stalled_clock_polls": 20,
        "rng_override": false, "gameplay_sql": false
    });
    let source = match Source::load(&arguments.source_root) {
        Ok(source) => source,
        Err(error) => {
            evidence.report["status"] = json!("blocked");
            evidence.report["first_failure"] =
                json!({"phase":"source_inputs", "reason":format!("{error:#}")});
            evidence.flush()?;
            return Err(error);
        }
    };
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
        event_ids: BTreeSet::new(),
        sequence: 0,
        input_count: 0,
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
    };
    let result = runner.execute().await;
    runner.evidence.report["input_count"] = json!(runner.input_count);
    runner.evidence.report["unique_events_observed"] = json!(runner.event_ids.len());
    runner.evidence.report["last_sequence"] = json!(runner.sequence.saturating_sub(1));
    if runner.snapshot.player.is_some() {
        runner.evidence.report["last_snapshot"] = public_snapshot(&runner.snapshot)?;
    }
    match &result {
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

    async fn rpc(&self, command: Command, token: bool) -> Result<Outcome> {
        self.bound()?;
        let (status, result) = self
            .connection
            .request(command, token.then_some(self.token.as_str()))
            .await?;
        if let Outcome::Error(error) = &result {
            bail!(
                "Real server rejected RPC: HTTP {status}, code={}, error_id={}, reason={}, retry_after_seconds={}; no replacement operation submitted",
                error.code,
                error.error_id,
                error.message,
                error.retry_after_seconds
            );
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
        let session = crate::login(&self.connection, &self.login_name, &self.password).await?;
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
        ensure!(
            !snapshot.event_history_gap,
            "Authoritative event history gap: acceptance evidence is incomplete"
        );
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
            self.input_count < self.arguments.max_inputs,
            "Journey input budget exhausted"
        );
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
                self.capture(
                    result.snapshot.context("Action result snapshot missing")?,
                    "action_ack",
                )?;
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
                bail!(
                    "Real game action rejected: HTTP {status}, code={}, error_id={}, reason={}",
                    error.code,
                    error.error_id,
                    error.message
                );
            }
            _ => bail!("WorldInput returned an unexpected Protobuf result/status {status}"),
        }
        Ok(())
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
        plan::validate_source_path(&self.source)?;
        self.register_and_join().await?;
        self.tutorial().await?;
        self.lumbridge().await?;
        self.cooks_assistant().await?;
        let receipt = self
            .reward_receipt
            .clone()
            .context("Missing acknowledged Cook reward receipt")?;
        self.recovery_checkpoint("after_quest", &receipt).await?;
        self.logout().await?;
        Ok(())
    }
}

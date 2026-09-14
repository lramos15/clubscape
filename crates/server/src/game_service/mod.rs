mod content;
mod random;
mod view;

#[cfg(test)]
mod tests;

use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    sync::{Arc, Mutex},
    time::Duration,
};

use axum::http::StatusCode;
use clubscape_game_types::{ActorId, GameError, GameErrorCode, TICK_MILLISECONDS};
use clubscape_protocol::{ErrorCode, client_message, game, server_message};
use prost::Message;
use sqlx::PgPool;
use tokio::{
    sync::{mpsc, oneshot, watch},
    task::{JoinHandle, JoinSet},
    time::{Instant, MissedTickBehavior, interval_at, timeout},
};
use uuid::Uuid;

use crate::{
    Config, ServeError, StartupError,
    error::ApiError,
    game_storage::{
        AuthTokenDigest, CommittedActorEvent, GameCommand, GameStore, SessionAccess,
        SourceCharacter, WorldLease, WorldSnapshot,
    },
    web_assets::WebAssets,
};
use content::LoadedContent;
use random::TrustedRandom;

const QUEUE_CAPACITY: usize = 64;
const INPUTS_PER_TICK: usize = 8;
const REQUEST_TIMEOUT: Duration = Duration::from_secs(9);
const OWNER_LEASE: Duration = Duration::from_secs(30);
const PLAYER_LEASE: Duration = Duration::from_secs(30);
const HISTORY_EVENTS: usize = 1024;
const HISTORY_BYTES: usize = 4 * 1024 * 1024;
const BASELINE_BYTES: usize = 8 * 1024 * 1024;
const PRESENCE_GAP: &str =
    "Source presence/logout handling is missing; the world cannot advance disconnected actors.";

type Reply = Result<server_message::Result, ApiError>;

struct Envelope {
    operation: Uuid,
    request_id: String,
    authentication: AuthTokenDigest,
    command: client_message::Command,
    reply: oneshot::Sender<Reply>,
}

#[derive(Clone)]
enum State {
    Ready,
    AwaitingPresence,
    Failed(ApiError),
    Stopping,
}

#[derive(Clone)]
pub(crate) struct GameHandle {
    sender: mpsc::Sender<Envelope>,
    state: Arc<Mutex<State>>,
    pub(crate) assets: Arc<WebAssets>,
    store: GameStore,
    world_id: Uuid,
}

impl GameHandle {
    pub(crate) fn availability(&self) -> Result<(), ApiError> {
        match &*self
            .state
            .lock()
            .map_err(|_| ApiError::internal("game_status_lock"))?
        {
            State::Ready => Ok(()),
            State::AwaitingPresence => Err(view::unavailable(PRESENCE_GAP)),
            State::Failed(error) => Err(error.clone()),
            State::Stopping => Err(view::unavailable("The world coordinator is stopping.")),
        }
    }

    pub(crate) async fn character_initialized(&self, digest: [u8; 32]) -> Result<bool, ApiError> {
        self.store
            .load_character(self.world_id, AuthTokenDigest::from_digest(digest))
            .await
            .map(|character| character.is_some())
            .map_err(Into::into)
    }

    pub(crate) async fn allow_account_logout(&self, digest: [u8; 32]) -> Result<(), ApiError> {
        if self.character_initialized(digest).await? {
            return Err(view::unavailable(
                "The source presence/logout transition API is missing; game-character logout cannot be acknowledged.",
            ));
        }
        Ok(())
    }

    pub(crate) async fn request(
        &self,
        operation: Uuid,
        request_id: &str,
        digest: [u8; 32],
        command: client_message::Command,
    ) -> Reply {
        let (reply, receive) = oneshot::channel();
        self.sender
            .try_send(Envelope {
                operation,
                request_id: request_id.to_owned(),
                authentication: AuthTokenDigest::from_digest(digest),
                command,
                reply,
            })
            .map_err(|error| match error {
                mpsc::error::TrySendError::Full(_) => capacity(),
                mpsc::error::TrySendError::Closed(_) => {
                    view::unavailable("The world coordinator is not running.")
                }
            })?;
        timeout(REQUEST_TIMEOUT, receive)
            .await
            .map_err(|_| {
                ApiError::deadline(
                    "game_coordinator",
                    "queue_or_execution",
                    true,
                    REQUEST_TIMEOUT,
                )
            })?
            .map_err(|_| {
                view::unavailable("The game request was cancelled; its outcome may be unknown.")
            })?
    }
}

pub(crate) struct PreparedGame {
    coordinator: Coordinator,
    pub(crate) handle: GameHandle,
}

impl PreparedGame {
    pub(crate) fn load(config: &Config) -> Result<Option<LoadedContent>, StartupError> {
        content::load(config)
    }

    pub(crate) async fn initialize(
        content: LoadedContent,
        pool: PgPool,
    ) -> Result<Self, StartupError> {
        let store = GameStore::new(pool);
        let initial = content
            .engine
            .initial_world()
            .map_err(|_| StartupError::new("game_world", "source_initial_world"))?;
        let world = store
            .initialize_world(content.world_id, initial)
            .await
            .map_err(|_| StartupError::new("game_world", "world_initialization"))?;
        world
            .state
            .validate_runtime(content.compiled.definition())
            .map_err(|_| StartupError::new("game_world", "restored_world_validation"))?;
        let lease = store
            .acquire_world_lease(content.world_id, OWNER_LEASE)
            .await
            .map_err(|_| StartupError::new("game_world", "world_ownership"))?;
        let key = match store
            .runtime_key(&lease, content.artifact_hash.clone())
            .await
        {
            Ok(key) => key,
            Err(_) => {
                if let Err(error) = store.release_world_lease(&lease).await {
                    tracing::error!(event = "game_startup_cleanup", error_id = %error.error_id, "world lease release failed");
                }
                return Err(StartupError::new("game_world", "runtime_identity"));
            }
        };
        let state = Arc::new(Mutex::new(if world.state.characters.is_empty() {
            State::Ready
        } else {
            State::AwaitingPresence
        }));
        let (sender, receiver) = mpsc::channel(QUEUE_CAPACITY);
        let handle = GameHandle {
            sender,
            state: state.clone(),
            assets: content.assets.clone(),
            store: store.clone(),
            world_id: content.world_id,
        };
        Ok(Self {
            coordinator: Coordinator {
                content,
                store,
                lease,
                world,
                key,
                receiver,
                state,
                pending: VecDeque::new(),
                sessions: BTreeMap::new(),
                history: VecDeque::new(),
                history_size: 0,
                history_count: 0,
                history_floor: 0,
                baselines: VecDeque::new(),
                baseline_size: 0,
            },
            handle,
        })
    }

    pub(crate) fn start(self) -> GameTask {
        let (shutdown, receive) = watch::channel(false);
        let state = self.coordinator.state.clone();
        let task = tokio::spawn(async move {
            let mut owned = JoinSet::new();
            owned.spawn(self.coordinator.run(receive));
            match owned.join_next().await {
                Some(Ok(result)) => result,
                _ => {
                    let error = ApiError::internal("game_coordinator_worker");
                    if let Ok(mut state) = state.lock() {
                        *state = State::Failed(error);
                    }
                    Err(ServeError::new("game_coordinator_worker"))
                }
            }
        });
        GameTask { shutdown, task }
    }
}

pub(crate) struct GameTask {
    shutdown: watch::Sender<bool>,
    task: JoinHandle<Result<(), ServeError>>,
}

impl GameTask {
    pub(crate) fn stop_signal(&self) -> watch::Sender<bool> {
        self.shutdown.clone()
    }

    pub(crate) async fn finish(mut self) -> Result<(), ServeError> {
        self.shutdown.send_replace(true);
        match timeout(Duration::from_secs(6), &mut self.task).await {
            Ok(Ok(result)) => result,
            Ok(Err(_)) => Err(ServeError::new("game_coordinator_worker")),
            Err(_) => {
                self.task.abort();
                let _ = timeout(Duration::from_secs(1), &mut self.task).await;
                Err(ServeError::new("game_coordinator_shutdown_deadline"))
            }
        }
    }
}

impl Drop for GameTask {
    fn drop(&mut self) {
        self.task.abort();
    }
}

struct Joined {
    access: SessionAccess,
    join_revision: u64,
}

struct History {
    revision: u64,
    events: Vec<CommittedActorEvent>,
    size: usize,
}

struct Baseline {
    session: Uuid,
    revision: u64,
    entities: BTreeMap<String, game::Entity>,
    size: usize,
}

struct Coordinator {
    content: LoadedContent,
    store: GameStore,
    lease: WorldLease,
    world: WorldSnapshot,
    key: [u8; 32],
    receiver: mpsc::Receiver<Envelope>,
    state: Arc<Mutex<State>>,
    pending: VecDeque<Envelope>,
    sessions: BTreeMap<ActorId, Joined>,
    history: VecDeque<History>,
    history_size: usize,
    history_count: usize,
    history_floor: u64,
    baselines: VecDeque<Baseline>,
    baseline_size: usize,
}

impl Coordinator {
    async fn run(mut self, mut shutdown: watch::Receiver<bool>) -> Result<(), ServeError> {
        self.history_floor = self.world.state.revision;
        let cadence = Duration::from_millis(TICK_MILLISECONDS);
        let mut ticks = interval_at(Instant::now() + cadence, cadence);
        ticks.set_missed_tick_behavior(MissedTickBehavior::Delay);
        let mut renew = interval_at(
            Instant::now() + Duration::from_secs(10),
            Duration::from_secs(10),
        );
        renew.set_missed_tick_behavior(MissedTickBehavior::Delay);
        loop {
            tokio::select! {
                biased;
                _ = shutdown.changed() => break,
                _ = ticks.tick() => {
                    tokio::select! {
                        biased;
                        _ = shutdown.changed() => break,
                        result = self.boundary() => {
                            if let Err(error) = result { self.fail(error); }
                        }
                    }
                },
                _ = renew.tick() => {
                    if self.failure().is_some() { continue; }
                    tokio::select! {
                        biased;
                        _ = shutdown.changed() => break,
                        result = self.store.renew_world_lease(&self.lease, OWNER_LEASE) => {
                            match result { Ok(lease) => self.lease = lease, Err(error) => self.fail(error.into()) }
                        }
                    }
                },
                request = self.receiver.recv() => {
                    let Some(request) = request else { break };
                    if request.reply.is_closed() { continue; }
                    if matches!(request.command, client_message::Command::WorldInput(_)) {
                        if let Some(error) = self.failure() {
                            let _ = request.reply.send(Err(error));
                        } else if self.pending.len() >= QUEUE_CAPACITY {
                            let _ = request.reply.send(Err(capacity()));
                        } else {
                            self.pending.push_back(request);
                        }
                    } else {
                        tokio::select! {
                            biased;
                            _ = shutdown.changed() => break,
                            _ = self.handle(request) => {}
                        }
                    }
                }
            }
        }
        let failed = self.failure().is_some();
        self.set_state(State::Stopping);
        self.receiver.close();
        while let Some(request) = self.pending.pop_front() {
            let _ = request.reply.send(Err(view::unavailable(
                "The game request was cancelled during shutdown; its outcome may be unknown.",
            )));
        }
        while let Ok(request) = self.receiver.try_recv() {
            let _ = request
                .reply
                .send(Err(view::unavailable("The world coordinator is stopping.")));
        }
        self.store
            .release_world_lease(&self.lease)
            .await
            .map_err(|_| ServeError::new("game_lease_release"))?;
        if failed {
            Err(ServeError::new("game_loop_failure"))
        } else {
            Ok(())
        }
    }

    fn set_state(&self, state: State) {
        match self.state.lock() {
            Ok(mut current) => *current = state,
            Err(_) => {
                ApiError::internal("game_status_lock");
            }
        }
    }

    fn fail(&mut self, error: ApiError) {
        tracing::error!(event = "game_loop_failure", error_id = %error.error_id,
            world_id = %self.content.world_id, code = ?error.code, "world processing stopped");
        self.set_state(State::Failed(error.clone()));
        while let Some(request) = self.pending.pop_front() {
            let _ = request.reply.send(Err(error.clone()));
        }
    }

    fn failure(&self) -> Option<ApiError> {
        match self.state.lock() {
            Ok(state) => match &*state {
                State::Failed(error) => Some(error.clone()),
                _ => None,
            },
            Err(_) => Some(ApiError::internal("game_status_lock")),
        }
    }

    async fn boundary(&mut self) -> Result<(), ApiError> {
        if self.failure().is_some() {
            return Ok(());
        }
        if !self.world.state.characters.keys().eq(self.sessions.keys()) {
            self.set_state(State::AwaitingPresence);
            for _ in 0..INPUTS_PER_TICK {
                if let Some(request) = self.pending.pop_front() {
                    let _ = request.reply.send(Err(view::unavailable(PRESENCE_GAP)));
                }
            }
            return Ok(());
        }
        self.set_state(State::Ready);
        let expected = self.world.state.tick;
        let mut random = TrustedRandom::tick(self.key, self.content.world_id, expected + 1);
        let engine = self.content.engine.clone();
        let compiled = self.content.compiled.clone();
        let sessions = self
            .sessions
            .values()
            .map(|joined| joined.access.clone())
            .collect();
        let tick = self
            .store
            .commit_routed_tick(&self.lease, expected, sessions, move |world| {
                let events = engine.process_advanced_tick(world, &mut random)?;
                world.validate_runtime(compiled.definition())?;
                for character in world.characters.values() {
                    view::check_context(character)?;
                }
                Ok(events)
            })
            .await;
        match tick {
            Ok(tick) => {
                self.world = tick.snapshot;
                self.publish(
                    tick.commit.receipt.world_revision,
                    tick.commit.receipt.routed_events,
                );
            }
            Err(error) if error.message.contains("unknown") => {
                let world = self
                    .store
                    .load_world(self.content.world_id)
                    .await
                    .map_err(ApiError::from)?;
                if world.state.tick == expected + 1 {
                    let receipt = world
                        .last_tick
                        .as_ref()
                        .ok_or_else(|| ApiError::internal("game_tick_reconciliation"))?;
                    self.publish(receipt.world_revision, receipt.routed_events.clone());
                    self.world = world;
                } else {
                    return Err(error.into());
                }
            }
            Err(error) => return Err(error.into()),
        }
        let mut attempted = BTreeSet::new();
        for _ in 0..INPUTS_PER_TICK {
            let Some(request) = self.pending.pop_front() else {
                break;
            };
            if request.reply.is_closed() {
                continue;
            }
            let result = self.input(&request, &mut attempted).await;
            let unknown = result
                .as_ref()
                .err()
                .filter(|error| error.message.contains("unknown"))
                .cloned();
            let _ = request.reply.send(result);
            if let Some(error) = unknown {
                return Err(error);
            }
        }
        Ok(())
    }

    async fn handle(&mut self, request: Envelope) {
        let result = if let Some(error) = self.failure() {
            Err(error)
        } else {
            self.lifecycle(&request).await
        };
        let _ = request.reply.send(result);
    }

    async fn access(
        &self,
        authentication: AuthTokenDigest,
        session_id: &str,
    ) -> Result<SessionAccess, ApiError> {
        let character = self
            .store
            .load_character(self.content.world_id, authentication)
            .await
            .map_err(ApiError::from)?
            .ok_or_else(|| conflict("Create a source-defined character before joining."))?;
        Ok(SessionAccess {
            world_id: self.content.world_id,
            actor_id: character.state.actor_id,
            session_id: Uuid::parse_str(session_id)
                .map_err(|_| ApiError::invalid("Invalid game session ID."))?,
            authentication,
        })
    }

    async fn lifecycle(&mut self, request: &Envelope) -> Reply {
        match &request.command {
            client_message::Command::CreateCharacter(options) => {
                if !options.appearance.is_empty() || !options.experience_choice.is_empty() {
                    return Err(ApiError::invalid(
                        "Create without options; appearance and experience must be confirmed by source-game intents after joining.",
                    ));
                }
                let character = self
                    .store
                    .create_character(
                        &self.lease,
                        request.authentication,
                        SourceCharacter {
                            content_revision: self.content.compiled.definition().revision.clone(),
                            initial_state: self.content.compiled.definition().initial_state.clone(),
                            appearance: BTreeMap::new(),
                        },
                    )
                    .await
                    .map_err(ApiError::from)?;
                self.world = self
                    .store
                    .load_world(self.content.world_id)
                    .await
                    .map_err(ApiError::from)?;
                Ok(server_message::Result::CharacterCreated(
                    game::CharacterCreated {
                        actor_id: character.state.actor_id.to_string(),
                        content_revision: self.content.compiled.definition().revision.clone(),
                    },
                ))
            }
            client_message::Command::JoinWorld(_) => {
                let character = self
                    .store
                    .load_character(self.content.world_id, request.authentication)
                    .await
                    .map_err(ApiError::from)?
                    .ok_or_else(|| conflict("Create a source-defined character before joining."))?;
                let session = self
                    .store
                    .join_session(
                        self.content.world_id,
                        character.state.actor_id.clone(),
                        request.authentication,
                        PLAYER_LEASE,
                    )
                    .await
                    .map_err(ApiError::from)?;
                let access = session.access(request.authentication);
                let snapshot = self
                    .store
                    .session_snapshot(&access, None)
                    .await
                    .map_err(ApiError::from)?;
                self.world = snapshot.world;
                self.sessions.insert(
                    access.actor_id.clone(),
                    Joined {
                        access: access.clone(),
                        join_revision: self.world.state.revision,
                    },
                );
                let mut public = view::snapshot(
                    &self.content.compiled,
                    &self.world,
                    &access.actor_id,
                    snapshot.character.revision,
                )?;
                public.event_history_floor_revision = self.world.state.revision;
                self.remember(&access, &public);
                if self.world.state.characters.keys().eq(self.sessions.keys()) {
                    self.set_state(State::Ready);
                }
                Ok(server_message::Result::WorldJoined(game::WorldJoined {
                    world_session_id: session.session_id.to_string(),
                    next_sequence: snapshot.character.last_sequence + 1,
                    content_revision: self.content.compiled.definition().revision.clone(),
                    content_manifest_path: self.content.public_manifest.clone(),
                    snapshot: Some(public),
                }))
            }
            client_message::Command::PollWorld(poll) => {
                let access = self
                    .access(request.authentication, &poll.world_session_id)
                    .await?;
                let snapshot = self
                    .store
                    .session_snapshot(&access, Some(PLAYER_LEASE))
                    .await
                    .map_err(ApiError::from)?;
                self.check_join(&access)?;
                self.world = snapshot.world;
                if poll.after_revision > self.world.state.revision {
                    return Err(conflict(
                        "The requested world revision is ahead of authoritative state.",
                    ));
                }
                let public = self.frame(
                    &access,
                    snapshot.character.revision,
                    Some(poll.after_revision),
                )?;
                Ok(server_message::Result::WorldSnapshot(public))
            }
            client_message::Command::LeaveWorld(leave) => {
                let access = self
                    .access(request.authentication, &leave.world_session_id)
                    .await?;
                self.store
                    .read_session(&access)
                    .await
                    .map_err(ApiError::from)?;
                Err(view::unavailable(
                    "The source presence/logout transition API is missing; the world session was not released.",
                ))
            }
            _ => Err(ApiError::invalid(
                "This is not a world-coordinator operation.",
            )),
        }
    }

    fn check_join(&self, access: &SessionAccess) -> Result<(), ApiError> {
        if self.sessions.get(&access.actor_id).is_none_or(|joined| {
            joined.access.session_id != access.session_id
                || joined.access.authentication != access.authentication
        }) {
            return Err(conflict(
                "Rejoin this coordinator before submitting world requests.",
            ));
        }
        Ok(())
    }

    async fn input(&mut self, request: &Envelope, attempted: &mut BTreeSet<ActorId>) -> Reply {
        let client_message::Command::WorldInput(input) = &request.command else {
            return Err(ApiError::invalid("A queued world intent is required."));
        };
        let access = self
            .access(request.authentication, &input.world_session_id)
            .await?;
        self.check_join(&access)?;
        let intent = clubscape_protocol::game_intent(input)?;
        // Authentication/ownership is checked again by the committing transaction.
        self.store
            .heartbeat_session(&access, PLAYER_LEASE)
            .await
            .map_err(ApiError::from)?;
        view::preflight(self.content.compiled.definition(), &intent)?;
        if request.reply.is_closed() {
            return Err(view::unavailable(
                "The queued game request was cancelled before mutation.",
            ));
        }
        let permit = attempted.insert(access.actor_id.clone());
        let engine = self.content.engine.clone();
        let compiled = self.content.compiled.clone();
        let mut random = TrustedRandom::command(self.key, self.content.world_id, request.operation);
        let committed = self
            .store
            .commit_routed_command(
                &self.lease,
                &access,
                GameCommand {
                    operation_id: request.operation,
                    sequence: input.sequence,
                    intent,
                },
                input.expected_character_revision,
                move |world, actor, intent| {
                    if !permit {
                        return Err(GameError::new(
                            GameErrorCode::Busy,
                            "Only one new intent attempt per actor per source tick.",
                        ));
                    }
                    let events = engine.apply_intent(world, actor, intent, &mut random)?;
                    world.validate_runtime(compiled.definition())?;
                    for character in world.characters.values() {
                        view::check_context(character)?;
                    }
                    Ok(events)
                },
            )
            .await
            .map_err(ApiError::from)?;
        self.world = committed.snapshot;
        if !committed.commit.duplicate {
            self.publish(
                committed.commit.receipt.world_revision,
                committed.commit.receipt.routed_events,
            );
        }
        let public = self.frame(&access, committed.character_revision, None)?;
        Ok(server_message::Result::ActionResult(game::ActionResult {
            sequence: input.sequence,
            operation_id: request.request_id.clone(),
            duplicate: committed.commit.duplicate,
            snapshot: Some(public),
        }))
    }

    fn publish(&mut self, revision: u64, events: Vec<CommittedActorEvent>) {
        if revision <= self.history_floor
            || self
                .history
                .back()
                .is_some_and(|last| last.revision >= revision)
        {
            return;
        }
        let size = events
            .iter()
            .map(|event| view::event(event).encoded_len())
            .sum();
        self.history_count += events.len();
        self.history_size += size;
        self.history.push_back(History {
            revision,
            events,
            size,
        });
        while self.history.len() > 256
            || self.history_count > HISTORY_EVENTS
            || self.history_size > HISTORY_BYTES
        {
            if let Some(old) = self.history.pop_front() {
                self.history_floor = old.revision;
                self.history_count -= old.events.len();
                self.history_size -= old.size;
            }
        }
    }

    fn frame(
        &mut self,
        access: &SessionAccess,
        character_revision: u64,
        after: Option<u64>,
    ) -> Result<game::WorldSnapshot, ApiError> {
        let joined = self
            .sessions
            .get(&access.actor_id)
            .ok_or_else(|| conflict("Rejoin the world."))?;
        let requested_floor = after.unwrap_or(joined.join_revision);
        let floor = requested_floor.max(joined.join_revision);
        let mut public = view::snapshot(
            &self.content.compiled,
            &self.world,
            &access.actor_id,
            character_revision,
        )?;
        public.event_history_floor_revision = self.history_floor.max(joined.join_revision);
        public.event_history_gap = requested_floor < public.event_history_floor_revision;
        let mut events: Vec<_> = self
            .history
            .iter()
            .filter(|batch| batch.revision > floor)
            .flat_map(|batch| &batch.events)
            .filter(|event| event.actor_id == access.actor_id)
            .map(view::event)
            .collect();
        if events.len() > 256 {
            public.event_history_gap = true;
            events.drain(..events.len() - 256);
        }
        public.events = events;
        if let Some(after) = after
            && !public.event_history_gap
            && let Some(baseline) = self
                .baselines
                .iter()
                .rev()
                .find(|entry| entry.session == access.session_id && entry.revision == after)
        {
            let complete: BTreeMap<_, _> = public
                .entities
                .iter()
                .map(|entity| (entity.id.clone(), entity.clone()))
                .collect();
            view::entity_delta(&mut public, &baseline.entities);
            view::bounded(&public)?;
            self.remember_entities(access.session_id, public.revision, complete);
            return Ok(public);
        }
        view::bounded(&public)?;
        self.remember(access, &public);
        Ok(public)
    }

    fn remember(&mut self, access: &SessionAccess, snapshot: &game::WorldSnapshot) {
        self.remember_entities(
            access.session_id,
            snapshot.revision,
            snapshot
                .entities
                .iter()
                .map(|entity| (entity.id.clone(), entity.clone()))
                .collect(),
        );
    }

    fn remember_entities(
        &mut self,
        session: Uuid,
        revision: u64,
        entities: BTreeMap<String, game::Entity>,
    ) {
        let size = entities.values().map(Message::encoded_len).sum();
        self.baseline_size += size;
        self.baselines.push_back(Baseline {
            session,
            revision,
            entities,
            size,
        });
        while self.baselines.len() > 128 || self.baseline_size > BASELINE_BYTES {
            if let Some(old) = self.baselines.pop_front() {
                self.baseline_size -= old.size;
            }
        }
    }
}

fn capacity() -> ApiError {
    let mut error = ApiError::new(
        StatusCode::TOO_MANY_REQUESTS,
        ErrorCode::ResourceExhausted,
        "The bounded world command queue is full; retry without changing an uncertain operation ID.",
    );
    error.retry_after_seconds = 1;
    error
}

fn conflict(message: &'static str) -> ApiError {
    ApiError::new(StatusCode::CONFLICT, ErrorCode::Conflict, message)
}

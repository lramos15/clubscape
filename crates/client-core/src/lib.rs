use std::collections::{BTreeMap, BTreeSet, VecDeque};

use clubscape_game_types::{ActorId, INVENTORY_SLOTS, ItemId, MAX_RUN_ENERGY, Quantity, Tile};
use clubscape_protocol::{
    Account, ClientMessage, ErrorCode, GAME_CAPABILITY, MAX_GAME_RESPONSE_BYTES, PROTOCOL_VERSION,
    ServerMessage, client_message::Command, game, server_message::Result as Outcome,
    validate_client_message,
};
use prost::Message;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Phase {
    Disconnected,
    Connecting,
    SigningIn,
    AccountReady,
    JoiningWorld,
    InWorld,
    Reconnecting,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ClientError {
    #[error("{0}")]
    InvalidState(&'static str),
    #[error("{0}")]
    InvalidResponse(&'static str),
    #[error("{0}")]
    InvalidInput(&'static str),
    #[error("Server rejected the request: {message} ({error_id})")]
    Server {
        code: ErrorCode,
        message: String,
        error_id: String,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub enum ClientEvent {
    AccountChanged,
    CharacterCreated(String),
    WorldChanged,
    Gameplay(Box<game::Event>),
    SignedOut,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum RequestKind {
    Hello,
    Register,
    Login,
    Account,
    Logout,
    Create,
    Join,
    Poll,
    Leave,
    Input { sequence: u64 },
}

#[derive(Clone)]
struct UncertainInput {
    request_id: String,
    input: game::WorldInput,
    outcome_unknown: bool,
}

pub struct ClientCore {
    phase: Phase,
    account: Option<Account>,
    token: Option<String>,
    capabilities: BTreeSet<String>,
    gameplay_available: bool,
    world_session: Option<String>,
    next_sequence: Option<u64>,
    snapshot: Option<game::WorldSnapshot>,
    pending: BTreeMap<String, RequestKind>,
    uncertain_input: Option<UncertainInput>,
    seen_events: BTreeSet<String>,
    event_order: VecDeque<String>,
    completed_requests: BTreeSet<String>,
    request_order: VecDeque<String>,
}

impl Default for ClientCore {
    fn default() -> Self {
        Self {
            phase: Phase::Disconnected,
            account: None,
            token: None,
            capabilities: BTreeSet::new(),
            gameplay_available: false,
            world_session: None,
            next_sequence: None,
            snapshot: None,
            pending: BTreeMap::new(),
            uncertain_input: None,
            seen_events: BTreeSet::new(),
            event_order: VecDeque::new(),
            completed_requests: BTreeSet::new(),
            request_order: VecDeque::new(),
        }
    }
}

impl std::fmt::Debug for ClientCore {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ClientCore")
            .field("phase", &self.phase)
            .field("authenticated", &self.token.is_some())
            .field("pending_requests", &self.pending.len())
            .field("has_uncertain_input", &self.uncertain_input.is_some())
            .finish()
    }
}

impl ClientCore {
    pub fn phase(&self) -> &Phase {
        &self.phase
    }
    pub fn account(&self) -> Option<&Account> {
        self.account.as_ref()
    }
    pub fn authorization_token(&self) -> Option<&str> {
        self.token.as_deref()
    }
    pub fn snapshot(&self) -> Option<&game::WorldSnapshot> {
        self.snapshot.as_ref()
    }
    pub fn next_sequence(&self) -> Option<u64> {
        self.next_sequence
    }

    pub fn prepare(
        &mut self,
        request_id: &str,
        command: Command,
    ) -> Result<ClientMessage, ClientError> {
        if self.pending.len() >= 32 {
            return Err(ClientError::InvalidState("Too many outstanding requests."));
        }
        if self.pending.contains_key(request_id) || self.completed_requests.contains(request_id) {
            return Err(ClientError::InvalidInput("Request IDs must be unique."));
        }
        let message = ClientMessage {
            protocol_version: PROTOCOL_VERSION,
            request_id: request_id.to_owned(),
            command: Some(command),
        };
        validate_client_message(&message)
            .map_err(|error| ClientError::InvalidInput(error.message))?;
        let kind = match message
            .command
            .as_ref()
            .expect("a validated command is present")
        {
            Command::Hello(_) => RequestKind::Hello,
            Command::Register(_) => RequestKind::Register,
            Command::Login(_) => RequestKind::Login,
            Command::CurrentAccount(_) => RequestKind::Account,
            Command::Logout(_) => RequestKind::Logout,
            Command::CreateCharacter(_) => RequestKind::Create,
            Command::JoinWorld(_) => RequestKind::Join,
            Command::PollWorld(poll) => {
                self.require_world_session(&poll.world_session_id)?;
                RequestKind::Poll
            }
            Command::LeaveWorld(leave) => {
                self.require_world_session(&leave.world_session_id)?;
                RequestKind::Leave
            }
            Command::WorldInput(_) => {
                return Err(ClientError::InvalidInput(
                    "Use submit_action to assign authoritative sequence/session.",
                ));
            }
        };
        if !matches!(
            kind,
            RequestKind::Hello | RequestKind::Register | RequestKind::Login
        ) && self.token.is_none()
        {
            return Err(ClientError::InvalidState(
                "Sign in before making this request.",
            ));
        }
        if matches!(kind, RequestKind::Register | RequestKind::Login) && self.token.is_some() {
            return Err(ClientError::InvalidState(
                "Sign out before starting another authentication flow.",
            ));
        }
        if matches!(kind, RequestKind::Create | RequestKind::Join)
            && (!self.gameplay_available || !self.capabilities.contains(GAME_CAPABILITY))
        {
            return Err(ClientError::InvalidState(
                "The server has not advertised gameplay support.",
            ));
        }
        match kind {
            RequestKind::Hello => self.phase = Phase::Connecting,
            RequestKind::Login => self.phase = Phase::SigningIn,
            RequestKind::Join => self.phase = Phase::JoiningWorld,
            _ => {}
        }
        self.pending.insert(request_id.to_owned(), kind);
        Ok(message)
    }

    pub fn submit_action(
        &mut self,
        request_id: &str,
        action: game::world_input::Action,
    ) -> Result<ClientMessage, ClientError> {
        if self.phase != Phase::InWorld {
            return Err(ClientError::InvalidState("A joined world is required."));
        }
        if self.pending.len() >= 32 {
            return Err(ClientError::InvalidState("Too many outstanding requests."));
        }
        if self.uncertain_input.is_some() {
            return Err(ClientError::InvalidState(
                "Resolve the outstanding game operation before submitting another.",
            ));
        }
        let input = game::WorldInput {
            world_session_id: self
                .world_session
                .clone()
                .ok_or(ClientError::InvalidState("World session is missing."))?,
            sequence: self
                .next_sequence
                .ok_or(ClientError::InvalidState("World sequence is missing."))?,
            expected_character_revision: self
                .snapshot
                .as_ref()
                .map(|snapshot| snapshot.character_revision),
            action: Some(action),
        };
        let message = ClientMessage {
            protocol_version: PROTOCOL_VERSION,
            request_id: request_id.to_owned(),
            command: Some(Command::WorldInput(input.clone())),
        };
        validate_client_message(&message)
            .map_err(|error| ClientError::InvalidInput(error.message))?;
        if self.pending.contains_key(request_id) || self.completed_requests.contains(request_id) {
            return Err(ClientError::InvalidInput("Request IDs must be unique."));
        }
        self.pending.insert(
            request_id.to_owned(),
            RequestKind::Input {
                sequence: input.sequence,
            },
        );
        self.uncertain_input = Some(UncertainInput {
            request_id: request_id.to_owned(),
            input,
            outcome_unknown: false,
        });
        Ok(message)
    }

    pub fn retry_uncertain_input(&mut self) -> Result<ClientMessage, ClientError> {
        if self.phase != Phase::InWorld {
            return Err(ClientError::InvalidState(
                "Rejoin the world before retrying an uncertain operation.",
            ));
        }
        let pending = self
            .uncertain_input
            .as_mut()
            .ok_or(ClientError::InvalidState(
                "There is no uncertain operation.",
            ))?;
        if !pending.outcome_unknown {
            return Err(ClientError::InvalidState(
                "The original operation is still awaiting its response.",
            ));
        }
        pending.input.world_session_id = self
            .world_session
            .clone()
            .ok_or(ClientError::InvalidState("World session is missing."))?;
        self.pending.insert(
            pending.request_id.clone(),
            RequestKind::Input {
                sequence: pending.input.sequence,
            },
        );
        pending.outcome_unknown = false;
        Ok(ClientMessage {
            protocol_version: PROTOCOL_VERSION,
            request_id: pending.request_id.clone(),
            command: Some(Command::WorldInput(pending.input.clone())),
        })
    }

    pub fn transport_lost(&mut self) {
        self.pending
            .retain(|_, kind| matches!(kind, RequestKind::Input { .. }));
        if let Some(pending) = &mut self.uncertain_input {
            pending.outcome_unknown = true;
        }
        self.phase = if self.token.is_some() {
            Phase::Reconnecting
        } else {
            Phase::Disconnected
        };
    }

    pub fn decode_response(&mut self, bytes: &[u8]) -> Result<Vec<ClientEvent>, ClientError> {
        if bytes.len() > MAX_GAME_RESPONSE_BYTES {
            return Err(ClientError::InvalidResponse(
                "Server response exceeds its byte budget.",
            ));
        }
        let message = ServerMessage::decode(bytes)
            .map_err(|_| ClientError::InvalidResponse("Server response is not valid Protobuf."))?;
        self.handle_response(message)
    }

    pub fn handle_response(
        &mut self,
        message: ServerMessage,
    ) -> Result<Vec<ClientEvent>, ClientError> {
        if message.protocol_version != PROTOCOL_VERSION {
            return Err(ClientError::InvalidResponse(
                "Unsupported response protocol version.",
            ));
        }
        if self.completed_requests.contains(&message.request_id) {
            return Ok(Vec::new());
        }
        let kind =
            self.pending
                .get(&message.request_id)
                .cloned()
                .ok_or(ClientError::InvalidResponse(
                    "Unsolicited or uncorrelated server response.",
                ))?;
        let result = message.result.ok_or(ClientError::InvalidResponse(
            "Server response has no result.",
        ))?;
        if let Outcome::Error(error) = result {
            let code = ErrorCode::try_from(error.code).map_err(|_| {
                ClientError::InvalidResponse("Server returned an unknown error code.")
            })?;
            if uuid::Uuid::parse_str(&error.error_id).is_err() {
                return Err(ClientError::InvalidResponse(
                    "Server error has no valid correlation ID.",
                ));
            }
            self.pending.remove(&message.request_id);
            if matches!(kind, RequestKind::Input { .. }) && code == ErrorCode::Unavailable {
                if let Some(pending) = &mut self.uncertain_input {
                    pending.outcome_unknown = true;
                }
                self.phase = Phase::Reconnecting;
            } else {
                if matches!(kind, RequestKind::Input { .. }) {
                    self.uncertain_input = None;
                }
                self.remember_request(message.request_id);
                self.phase = if self.world_session.is_some() {
                    Phase::InWorld
                } else if self.token.is_some() {
                    Phase::AccountReady
                } else {
                    Phase::Disconnected
                };
            }
            if code == ErrorCode::Unauthenticated {
                self.clear_authentication();
            }
            return Err(ClientError::Server {
                code,
                message: error.message,
                error_id: error.error_id,
            });
        }
        let mut events = Vec::new();
        match (kind, result) {
            (RequestKind::Hello, Outcome::Hello(hello)) => {
                if hello.gameplay_available
                    && !hello
                        .capabilities
                        .iter()
                        .any(|value| value == GAME_CAPABILITY)
                {
                    return Err(ClientError::InvalidResponse(
                        "Gameplay support lacks its protocol capability.",
                    ));
                }
                self.capabilities = hello.capabilities.into_iter().collect();
                self.gameplay_available = hello.gameplay_available;
                self.phase = if self.token.is_some() {
                    Phase::AccountReady
                } else {
                    Phase::Disconnected
                };
            }
            (RequestKind::Register, Outcome::Registered(registered)) => {
                validate_account(registered.account.as_ref())?;
            }
            (RequestKind::Login, Outcome::LoggedIn(login)) => {
                validate_account(login.account.as_ref())?;
                if login.session_token.len() != 43
                    || !login
                        .session_token
                        .bytes()
                        .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-')
                    || login.expires_at_unix_ms <= 0
                {
                    return Err(ClientError::InvalidResponse(
                        "Server returned an invalid authentication session.",
                    ));
                }
                self.account = login.account;
                self.token = Some(login.session_token);
                self.reset_world();
                self.phase = Phase::AccountReady;
                events.push(ClientEvent::AccountChanged);
            }
            (RequestKind::Account, Outcome::Account(snapshot)) => {
                validate_account(snapshot.account.as_ref())?;
                if self.account.as_ref().map(|account| &account.account_id)
                    != snapshot.account.as_ref().map(|account| &account.account_id)
                {
                    return Err(ClientError::InvalidResponse(
                        "Server changed the authenticated account identity.",
                    ));
                }
                self.account = snapshot.account;
                events.push(ClientEvent::AccountChanged);
            }
            (RequestKind::Logout, Outcome::LoggedOut(_)) => {
                self.clear_authentication();
                events.push(ClientEvent::SignedOut);
            }
            (RequestKind::Create, Outcome::CharacterCreated(created)) => {
                ActorId::new(&created.actor_id).map_err(|_| {
                    ClientError::InvalidResponse("Server returned an invalid actor ID.")
                })?;
                events.push(ClientEvent::CharacterCreated(created.actor_id));
            }
            (RequestKind::Join, Outcome::WorldJoined(joined)) => {
                clubscape_protocol::validate_world_session(&joined.world_session_id).map_err(
                    |_| ClientError::InvalidResponse("Server returned an invalid world session."),
                )?;
                if joined.next_sequence == 0 {
                    return Err(ClientError::InvalidResponse(
                        "Server returned an invalid next sequence.",
                    ));
                }
                let snapshot = joined.snapshot.ok_or(ClientError::InvalidResponse(
                    "Joined world lacks a snapshot.",
                ))?;
                validate_snapshot(&snapshot)?;
                if let Some(pending) = &self.uncertain_input {
                    if joined.next_sequence < pending.input.sequence {
                        return Err(ClientError::InvalidResponse(
                            "Server sequence regressed across reconnect.",
                        ));
                    }
                    if joined.next_sequence > pending.input.sequence {
                        let id = pending.request_id.clone();
                        self.pending.remove(&id);
                        self.remember_request(id);
                        self.uncertain_input = None;
                    }
                }
                self.world_session = Some(joined.world_session_id);
                self.next_sequence = Some(joined.next_sequence);
                self.snapshot = Some(snapshot);
                self.phase = Phase::InWorld;
                events.push(ClientEvent::WorldChanged);
            }
            (RequestKind::Poll, Outcome::WorldSnapshot(snapshot)) => {
                events.extend(self.apply_snapshot(snapshot)?);
            }
            (RequestKind::Input { sequence }, Outcome::ActionResult(result)) => {
                if result.sequence != sequence || result.operation_id != message.request_id {
                    return Err(ClientError::InvalidResponse(
                        "Game operation acknowledgement does not match its request.",
                    ));
                }
                let next = sequence
                    .checked_add(1)
                    .ok_or(ClientError::InvalidResponse("Game sequence overflowed."))?;
                let snapshot = result.snapshot.ok_or(ClientError::InvalidResponse(
                    "Game operation lacks authoritative state.",
                ))?;
                events.extend(self.apply_snapshot(snapshot)?);
                self.next_sequence = Some(next);
                self.uncertain_input = None;
                self.phase = Phase::InWorld;
            }
            (RequestKind::Leave, Outcome::WorldLeft(_)) => {
                self.reset_world();
                self.phase = Phase::AccountReady;
                events.push(ClientEvent::WorldChanged);
            }
            _ => {
                return Err(ClientError::InvalidResponse(
                    "Server result does not match the requested operation.",
                ));
            }
        }
        self.pending.remove(&message.request_id);
        self.remember_request(message.request_id);
        Ok(events)
    }

    fn apply_snapshot(
        &mut self,
        mut snapshot: game::WorldSnapshot,
    ) -> Result<Vec<ClientEvent>, ClientError> {
        validate_snapshot(&snapshot)?;
        let previous = self.snapshot.as_ref().ok_or(ClientError::InvalidState(
            "Join a world before applying state updates.",
        ))?;
        if snapshot.revision < previous.revision {
            return Ok(Vec::new());
        }
        if let (Some(before), Some(after)) = (&previous.player, &snapshot.player)
            && before.actor_id != after.actor_id
        {
            return Err(ClientError::InvalidResponse(
                "World update changed the local actor identity.",
            ));
        }
        if snapshot.character_revision < previous.character_revision {
            return Err(ClientError::InvalidResponse(
                "Character revision regressed inside a newer world update.",
            ));
        }
        if !snapshot.full_snapshot {
            let mut entities: BTreeMap<String, game::Entity> = previous
                .entities
                .iter()
                .map(|entity| (entity.id.clone(), entity.clone()))
                .collect();
            for removed in &snapshot.removed_entities {
                entities.remove(removed);
            }
            for entity in &snapshot.entities {
                entities.insert(entity.id.clone(), entity.clone());
            }
            snapshot.entities = entities.into_values().collect();
            if snapshot.entities.len() > 2048 {
                return Err(ClientError::InvalidResponse(
                    "Merged world update exceeds its entity bound.",
                ));
            }
        }
        let mut events = Vec::new();
        for event in &snapshot.events {
            if self.seen_events.insert(event.event_id.clone()) {
                self.event_order.push_back(event.event_id.clone());
                events.push(ClientEvent::Gameplay(Box::new(event.clone())));
            }
        }
        while self.event_order.len() > 1024 {
            if let Some(id) = self.event_order.pop_front() {
                self.seen_events.remove(&id);
            }
        }
        self.snapshot = Some(snapshot);
        events.push(ClientEvent::WorldChanged);
        Ok(events)
    }

    fn require_world_session(&self, session: &str) -> Result<(), ClientError> {
        if self.world_session.as_deref() != Some(session) {
            return Err(ClientError::InvalidState(
                "Request does not belong to the current world session.",
            ));
        }
        Ok(())
    }

    fn remember_request(&mut self, id: String) {
        if self.completed_requests.insert(id.clone()) {
            self.request_order.push_back(id);
        }
        while self.request_order.len() > 256 {
            if let Some(id) = self.request_order.pop_front() {
                self.completed_requests.remove(&id);
            }
        }
    }

    fn reset_world(&mut self) {
        self.world_session = None;
        self.next_sequence = None;
        self.snapshot = None;
        self.uncertain_input = None;
        self.seen_events.clear();
        self.event_order.clear();
    }

    fn clear_authentication(&mut self) {
        self.account = None;
        self.token = None;
        self.pending.clear();
        self.reset_world();
        self.phase = Phase::Disconnected;
    }
}

fn validate_account(account: Option<&Account>) -> Result<(), ClientError> {
    let account = account.ok_or(ClientError::InvalidResponse(
        "Server account identity is missing.",
    ))?;
    if !uuid::Uuid::parse_str(&account.account_id).is_ok_and(|id| !id.is_nil())
        || clubscape_protocol::normalize_login_name(&account.login_name).is_err()
    {
        return Err(ClientError::InvalidResponse(
            "Server returned an invalid account identity.",
        ));
    }
    Ok(())
}

fn validate_snapshot(snapshot: &game::WorldSnapshot) -> Result<(), ClientError> {
    let player = snapshot
        .player
        .as_ref()
        .ok_or(ClientError::InvalidResponse(
            "World state requires the authoritative local player.",
        ))?;
    ActorId::new(&player.actor_id)
        .map_err(|_| ClientError::InvalidResponse("Invalid player identity."))?;
    let tile = player
        .tile
        .as_ref()
        .ok_or(ClientError::InvalidResponse("Player position is missing."))?;
    let x = u16::try_from(tile.x)
        .map_err(|_| ClientError::InvalidResponse("Player position is out of range."))?;
    let y = u16::try_from(tile.y)
        .map_err(|_| ClientError::InvalidResponse("Player position is out of range."))?;
    let plane = u8::try_from(tile.plane)
        .map_err(|_| ClientError::InvalidResponse("Player plane is out of range."))?;
    Tile::new(x, y, plane)
        .map_err(|_| ClientError::InvalidResponse("Player position is invalid."))?;
    if player.run_energy > u32::from(MAX_RUN_ENERGY) {
        return Err(ClientError::InvalidResponse(
            "Player run energy is out of range.",
        ));
    }
    let mut slots = BTreeSet::new();
    for slot in &player.inventory {
        if slot.index >= INVENTORY_SLOTS as u32 || !slots.insert(slot.index) {
            return Err(ClientError::InvalidResponse(
                "Inventory slots are duplicated or out of range.",
            ));
        }
        let stack = slot
            .stack
            .as_ref()
            .ok_or(ClientError::InvalidResponse("Inventory stack is missing."))?;
        ItemId::new(&stack.item)
            .map_err(|_| ClientError::InvalidResponse("Inventory item ID is invalid."))?;
        Quantity::new(stack.quantity)
            .map_err(|_| ClientError::InvalidResponse("Inventory quantity is invalid."))?;
    }
    let mut event_ids = BTreeSet::new();
    if snapshot.events.len() > 256 || snapshot.entities.len() > 2048 {
        return Err(ClientError::InvalidResponse(
            "World update exceeds its entity/event bounds.",
        ));
    }
    for event in &snapshot.events {
        if event.event_id.is_empty()
            || event.event_id.len() > 192
            || !event_ids.insert(&event.event_id)
        {
            return Err(ClientError::InvalidResponse(
                "Game events require unique stable deduplication IDs.",
            ));
        }
    }
    let mut entity_ids = BTreeSet::new();
    for entity in &snapshot.entities {
        if entity.id.is_empty() || entity.id.len() > 192 || !entity_ids.insert(&entity.id) {
            return Err(ClientError::InvalidResponse(
                "Entity identities are invalid or duplicated.",
            ));
        }
    }
    Ok(())
}

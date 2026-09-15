pub mod catalog;
mod context;
pub mod gameplay_ui;
mod intent;
mod observer;
mod quote;
mod ui_input;
mod ui_wire;
mod view;

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use clubscape_client_core::{ClientCore, ClientError, ClientEvent, Phase};
use clubscape_protocol::{
    ClientMessage, ErrorCode, MAX_GAME_RESPONSE_BYTES, PROTOCOL_VERSION, ServerMessage,
    client_message::Command, game, server_message::Result as Outcome,
};
use prost::Message;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use wasm_bindgen::prelude::*;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BridgeError {
    kind: &'static str,
    message: String,
    error_id: Option<String>,
    code: Option<i32>,
    retry_after_seconds: u32,
    recoverable: bool,
}

impl BridgeError {
    fn new(kind: &'static str, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
            error_id: None,
            code: None,
            retry_after_seconds: 0,
            recoverable: kind != "protocol",
        }
    }
    fn input(message: &str) -> Self {
        Self::new("input", message)
    }
    fn protocol(message: &str) -> Self {
        Self::new("protocol", message)
    }
    fn js(self) -> JsValue {
        JsValue::from_str(&serde_json::to_string(&self).expect("error serialization"))
    }
}

impl From<ClientError> for BridgeError {
    fn from(value: ClientError) -> Self {
        match value {
            ClientError::InvalidState(message) => Self::new("state", message),
            ClientError::InvalidResponse(message) => Self::protocol(message),
            ClientError::InvalidInput(message) => Self::input(message),
            ClientError::Server {
                code,
                message,
                error_id,
            } => Self {
                kind: "server",
                message,
                error_id: Some(error_id),
                code: Some(code as i32),
                retry_after_seconds: 0,
                recoverable: !matches!(code, ErrorCode::UnsupportedVersion),
            },
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Credentials {
    login_name: String,
    password: String,
}

#[derive(Clone)]
struct PendingLifecycle {
    request_id: String,
    command: Command,
    uncertain: bool,
}

#[derive(Default)]
pub struct Bridge {
    core: ClientCore,
    session: Option<String>,
    character_initialized: bool,
    content_revision: Option<String>,
    manifest_path: Option<String>,
    gameplay_available: bool,
    capabilities: BTreeSet<String>,
    unavailable_reason: Option<String>,
    server_build: Option<String>,
    uncertain_sequence: Option<u64>,
    uncertain_request: Option<String>,
    catalog: Option<catalog::DisplayCatalog>,
    applied: BTreeSet<String>,
    applied_order: VecDeque<String>,
    messages: Vec<Value>,
    seen_events: BTreeSet<String>,
    event_order: VecDeque<String>,
    pending_quotes: BTreeMap<String, quote::Selection>,
    pending_lifecycle: Option<PendingLifecycle>,
}

impl Bridge {
    fn validate_snapshot_contract(
        &self,
        snapshot: &game::WorldSnapshot,
    ) -> Result<(), BridgeError> {
        if self.capabilities.contains(gameplay_ui::CAPABILITY) != snapshot.ui.is_some() {
            return Err(BridgeError::protocol(
                "game.ui.v1 negotiation requires a complete versioned UI view, never an unversioned fallback.",
            ));
        }
        if let Some(ui) = &snapshot.ui {
            ui_wire::decode(ui)?;
        }
        let player = snapshot
            .player
            .as_ref()
            .ok_or_else(|| BridgeError::protocol("A world snapshot has no local player."))?;
        let observer = self.capabilities.contains(observer::CAPABILITY);
        if observer && player.running.is_none() {
            return Err(BridgeError::protocol(
                "The negotiated actor observer omitted actual local movement state.",
            ));
        }
        if !observer
            && (player.running.is_some()
                || player.movement_tick.is_some()
                || player.action.is_some())
        {
            return Err(BridgeError::protocol(
                "Actor observer fields require game.observer.v1 negotiation.",
            ));
        }
        observer::fields(
            player.running,
            &player.movement_tick,
            player.action.as_ref(),
        )?;
        for entity in &snapshot.entities {
            if observer
                && entity.kind == game::EntityKind::Player as i32
                && entity.running.is_none()
            {
                return Err(BridgeError::protocol(
                    "The negotiated observer omitted visible-player movement state.",
                ));
            }
            if !observer
                && (entity.running.is_some()
                    || entity.movement_tick.is_some()
                    || entity.action.is_some())
            {
                return Err(BridgeError::protocol(
                    "Entity observer fields require game.observer.v1 negotiation.",
                ));
            }
            observer::fields(
                entity.running,
                &entity.movement_tick,
                entity.action.as_ref(),
            )?;
        }
        Ok(())
    }

    pub fn prepare(
        &mut self,
        request_id: &str,
        operation: &str,
        input: &str,
    ) -> Result<Vec<u8>, BridgeError> {
        if input.len() > clubscape_protocol::MAX_REQUEST_BYTES {
            return Err(BridgeError::input(
                "The account input exceeds its byte budget.",
            ));
        }
        let selection = if operation == "quote" {
            if self.catalog.is_none() {
                return Err(BridgeError::new(
                    "state",
                    "Load the joined source catalog before requesting a quote.",
                ));
            }
            Some(quote::Selection::parse(input)?)
        } else {
            None
        };
        let command = match operation {
            "hello" => Command::Hello(clubscape_protocol::Hello {}),
            "register" | "login" => {
                let credentials: Credentials = serde_json::from_str(input)
                    .map_err(|_| BridgeError::input("Invalid credential input shape."))?;
                if operation == "register" {
                    Command::Register(clubscape_protocol::Register {
                        login_name: credentials.login_name,
                        password: credentials.password,
                    })
                } else {
                    Command::Login(clubscape_protocol::Login {
                        login_name: credentials.login_name,
                        password: credentials.password,
                    })
                }
            }
            "account" => Command::CurrentAccount(clubscape_protocol::CurrentAccount {}),
            "logout" => Command::Logout(clubscape_protocol::Logout {}),
            "create_character" => {
                if input != "{}" {
                    return Err(BridgeError::input(
                        "Character creation must use the source initial state. Confirm appearance after joining.",
                    ));
                }
                Command::CreateCharacter(game::CreateCharacter::default())
            }
            "join" => Command::JoinWorld(game::JoinWorld {}),
            "poll" | "quote" => Command::PollWorld(game::PollWorld {
                world_session_id: self.session.clone().ok_or_else(|| {
                    BridgeError::new("state", "Join the world before requesting updates.")
                })?,
                after_revision: self.core.snapshot().map_or(0, |snapshot| snapshot.revision),
                quote: selection.as_ref().map(quote::Selection::wire).transpose()?,
            }),
            "leave" => Command::LeaveWorld(game::LeaveWorld {
                world_session_id: self.session.clone().ok_or_else(|| {
                    BridgeError::new("state", "There is no joined world to leave.")
                })?,
            }),
            _ => return Err(BridgeError::input("Unsupported bridge operation.")),
        };
        let lifecycle = matches!(command, Command::LeaveWorld(_) | Command::Logout(_));
        if matches!(command, Command::CreateCharacter(_) | Command::JoinWorld(_))
            && self.capabilities.contains(gameplay_ui::CAPABILITY)
            && !gameplay_ui::WIRE_SUPPORTED
        {
            return Err(BridgeError::new(
                "unsupported_protocol",
                "The server advertises game.ui.v1, but this client has no generated gameplay UI wire decoder yet. No character/world request was sent.",
            ));
        }
        if self.pending_lifecycle.is_some()
            && (lifecycle || matches!(command, Command::JoinWorld(_)))
        {
            return Err(BridgeError::new(
                "state",
                "Resolve the outstanding lifecycle operation before leaving, signing out or rejoining.",
            ));
        }
        let message = self.core.prepare(request_id, command)?;
        if lifecycle {
            self.pending_lifecycle = Some(PendingLifecycle {
                request_id: request_id.to_owned(),
                command: message.command.clone().expect("prepared lifecycle command"),
                uncertain: false,
            });
        }
        if let Some(selection) = selection {
            self.pending_quotes.insert(request_id.to_owned(), selection);
        }
        Ok(message.encode_to_vec())
    }

    pub fn submit(&mut self, request_id: &str, input: &str) -> Result<Vec<u8>, BridgeError> {
        if input.len() > clubscape_protocol::MAX_REQUEST_BYTES {
            return Err(BridgeError::input(
                "The UI/game input exceeds its protocol byte budget.",
            ));
        }
        if let Some((request, bank_revision)) = ui_input::parse(input)? {
            if !self.capabilities.contains(gameplay_ui::CAPABILITY) {
                return Err(BridgeError::new(
                    "unsupported_capability",
                    "This server has not advertised game.ui.v1. No UI request was sent.",
                ));
            }
            if self
                .core
                .snapshot()
                .and_then(|snapshot| snapshot.ui.as_ref())
                .is_none()
            {
                return Err(BridgeError::protocol(
                    "The negotiated UI view is missing; no UI request was sent.",
                ));
            }
            return self.submit_action(
                request_id,
                game::world_input::Action::Ui(ui_input::wire(request, bank_revision)?),
            );
        }
        let action = intent::action(input)?;
        self.submit_action(request_id, action)
    }

    fn submit_action(
        &mut self,
        request_id: &str,
        action: game::world_input::Action,
    ) -> Result<Vec<u8>, BridgeError> {
        if matches!(&action, game::world_input::Action::ShopBuy(buy) if buy.expected_item.is_none())
        {
            return Err(BridgeError::input(
                "New browser purchases require the displayed row's canonical expected_item. No purchase was sent.",
            ));
        }
        if self
            .core
            .snapshot()
            .and_then(|snapshot| snapshot.player.as_ref())
            .and_then(|player| player.presence.as_ref())
            .is_some_and(|presence| !presence.accepts_input)
        {
            return Err(BridgeError::new(
                "state",
                "The authoritative presence view does not currently accept game input.",
            ));
        }
        let message = self.core.submit_action(request_id, action)?;
        if let Some(Command::WorldInput(input)) = &message.command {
            self.uncertain_sequence = Some(input.sequence);
            self.uncertain_request = Some(request_id.to_owned());
        }
        Ok(message.encode_to_vec())
    }

    pub fn submit_selected(
        &mut self,
        request_id: &str,
        input: &str,
        item_id: &str,
    ) -> Result<Vec<u8>, BridgeError> {
        clubscape_game_types::ItemId::new(item_id)
            .map_err(|_| BridgeError::input("A purchase must retain the selected ItemId."))?;
        let mut action = intent::action(input)?;
        let game::world_input::Action::ShopBuy(buy) = &mut action else {
            return Err(BridgeError::input(
                "Selected-item submission is reserved for shop purchases.",
            ));
        };
        if buy
            .expected_item
            .as_ref()
            .is_some_and(|expected| expected != item_id)
        {
            return Err(BridgeError::input(
                "Purchase identity fields disagree; no identity was replaced.",
            ));
        }
        buy.expected_item = Some(item_id.to_owned());
        self.submit_action(request_id, action)
    }

    pub fn retry(&mut self) -> Result<Option<Vec<u8>>, BridgeError> {
        if self.uncertain_sequence.is_none() {
            return Ok(None);
        }
        self.core
            .retry_uncertain_input()
            .map(|message| Some(message.encode_to_vec()))
            .map_err(Into::into)
    }

    pub fn retry_lifecycle(&mut self) -> Result<Option<Vec<u8>>, BridgeError> {
        let Some(pending) = self.pending_lifecycle.as_ref() else {
            return Ok(None);
        };
        if !pending.uncertain {
            return Err(BridgeError::new(
                "state",
                "The lifecycle operation is still awaiting its original response.",
            ));
        }
        let message = self
            .core
            .prepare(&pending.request_id, pending.command.clone())?;
        self.pending_lifecycle
            .as_mut()
            .expect("pending lifecycle")
            .uncertain = false;
        Ok(Some(message.encode_to_vec()))
    }

    pub fn transport_lost(&mut self) {
        self.pending_quotes.clear();
        if let Some(pending) = &mut self.pending_lifecycle {
            pending.uncertain = true;
        }
        self.core.transport_lost();
    }

    pub fn authorization(&self) -> Option<&str> {
        self.core.authorization_token()
    }

    pub fn set_catalog(&mut self, input: &str) -> Result<String, BridgeError> {
        if input.len() > 8 * 1024 * 1024 {
            return Err(BridgeError::input(
                "The public display catalog exceeds its byte budget.",
            ));
        }
        let catalog: catalog::DisplayCatalog = serde_json::from_str(input)
            .map_err(|_| BridgeError::protocol("Invalid public display catalog."))?;
        if self.content_revision.as_deref() != Some(catalog.content_revision.as_str()) {
            return Err(BridgeError::protocol(
                "The loaded display catalog does not match the joined server content revision.",
            ));
        }
        if let Some(snapshot) = self.core.snapshot() {
            view::world(snapshot, &catalog, &self.messages)?;
        }
        self.catalog = Some(catalog);
        self.state()
    }

    pub fn receive(&mut self, bytes: &[u8]) -> Result<String, BridgeError> {
        self.receive_checked(None, bytes)
    }

    pub fn receive_for(&mut self, request: &str, bytes: &[u8]) -> Result<String, BridgeError> {
        self.receive_checked(Some(request), bytes)
    }

    fn receive_checked(
        &mut self,
        request: Option<&str>,
        bytes: &[u8],
    ) -> Result<String, BridgeError> {
        if bytes.len() > MAX_GAME_RESPONSE_BYTES {
            return Err(BridgeError::protocol(
                "The response exceeds the protocol byte budget.",
            ));
        }
        let message = ServerMessage::decode(bytes)
            .map_err(|_| BridgeError::protocol("The server response is not valid Protobuf."))?;
        if request.is_some_and(|request| request != message.request_id) {
            return Err(BridgeError::protocol(
                "The HTTP response does not belong to this exact RPC operation.",
            ));
        }
        if message.protocol_version != PROTOCOL_VERSION {
            return Err(BridgeError::protocol(
                "Unsupported response protocol version.",
            ));
        }
        if self.applied.contains(&message.request_id) {
            return self.state_with_events(Vec::new());
        }
        let outcome = message.result.clone();
        let request_id = message.request_id.clone();
        if let Some(Outcome::WorldJoined(joined)) = &outcome {
            if !public_content_path(&joined.content_manifest_path)
                || joined.content_revision.is_empty()
                || joined.content_revision.len() > 256
            {
                return Err(BridgeError::protocol(
                    "World join lacks a valid same-origin content manifest/revision.",
                ));
            }
            if self.content_revision.as_ref().is_some_and(|revision| {
                revision != &joined.content_revision && self.uncertain_sequence.is_some()
            }) {
                return Err(BridgeError::protocol(
                    "Server content changed while a game operation had an unknown outcome.",
                ));
            }
        }
        let snapshot = match &outcome {
            Some(Outcome::WorldJoined(joined)) => joined.snapshot.as_ref(),
            Some(Outcome::WorldSnapshot(snapshot)) => Some(snapshot),
            Some(Outcome::ActionResult(result)) => result.snapshot.as_ref(),
            _ => None,
        };
        if let Some(snapshot) = snapshot {
            self.validate_snapshot_contract(snapshot)?;
        }
        if let (Some(snapshot), Some(catalog)) = (snapshot, self.catalog.as_ref()) {
            view::world(snapshot, catalog, &self.messages)?;
        }
        let mut quote_result = None;
        let mut quote_error = None;
        if let (Some(selection), Some(snapshot)) = (self.pending_quotes.get(&request_id), snapshot)
        {
            if self
                .core
                .snapshot()
                .is_some_and(|previous| snapshot.revision < previous.revision)
            {
                quote_error =
                    Some("The quote belongs to an older world revision. Request a fresh quote.");
            } else if let Some(value) = snapshot
                .quote
                .as_ref()
                .filter(|value| selection.matches(value))
            {
                quote_result = Some(context::quote(
                    value,
                    self.catalog
                        .as_ref()
                        .ok_or_else(|| BridgeError::protocol("Quote catalog is missing."))?,
                    snapshot.revision,
                    snapshot.tick,
                )?);
            } else {
                quote_error = Some(
                    "The source quote no longer matches the selected item, quantity or recovery identities. No transaction was sent.",
                );
            }
        }
        if let Some(Outcome::Error(error)) = &outcome
            && error.code == ErrorCode::Unavailable as i32
            && self
                .pending_lifecycle
                .as_ref()
                .is_some_and(|pending| pending.request_id == request_id)
        {
            if uuid::Uuid::parse_str(&error.error_id).is_err() {
                return Err(BridgeError::protocol(
                    "The lifecycle error has no valid correlation ID.",
                ));
            }
            // This additive lifecycle journal has the same unknown-outcome rule as game inputs.
            // Do not let an unavailable receipt become a completed request in older client-core.
            self.transport_lost();
            return Err(BridgeError {
                kind: "server",
                message: error.message.clone(),
                error_id: Some(error.error_id.clone()),
                code: Some(error.code),
                retry_after_seconds: error.retry_after_seconds.min(60),
                recoverable: true,
            });
        }
        let events = match self.core.handle_response(message) {
            Ok(events) => events,
            Err(error) => {
                if matches!(&error, ClientError::Server { .. }) {
                    self.pending_quotes.remove(&request_id);
                    if self
                        .pending_lifecycle
                        .as_ref()
                        .is_some_and(|pending| pending.request_id == request_id)
                    {
                        self.pending_lifecycle = None;
                    }
                }
                if self.core.authorization_token().is_none() {
                    self.clear_private_world();
                } else if matches!(&error, ClientError::Server { .. })
                    && self.core.phase() != &Phase::Reconnecting
                    && self.uncertain_request.as_deref() == Some(request_id.as_str())
                {
                    self.uncertain_sequence = None;
                    self.uncertain_request = None;
                }
                let mut error = BridgeError::from(error);
                if let Some(Outcome::Error(server)) = &outcome {
                    error.retry_after_seconds = server.retry_after_seconds.min(60);
                }
                return Err(error);
            }
        };
        self.pending_quotes.remove(&request_id);
        if self
            .pending_lifecycle
            .as_ref()
            .is_some_and(|pending| pending.request_id == request_id)
        {
            self.pending_lifecycle = None;
        }
        if self.applied.insert(request_id.clone()) {
            self.applied_order.push_back(request_id);
        }
        while self.applied_order.len() > 256 {
            if let Some(id) = self.applied_order.pop_front() {
                self.applied.remove(&id);
            }
        }
        match outcome {
            Some(Outcome::Hello(hello)) => {
                self.capabilities = hello.capabilities.into_iter().collect();
                self.gameplay_available = hello.gameplay_available;
                self.unavailable_reason = (!hello.gameplay_unavailable_reason.is_empty())
                    .then_some(hello.gameplay_unavailable_reason);
                self.server_build = Some(hello.build_revision);
            }
            Some(Outcome::Account(account)) => {
                self.character_initialized = account.character_initialized;
                if !account.gameplay_unavailable_reason.is_empty() {
                    self.unavailable_reason = Some(account.gameplay_unavailable_reason);
                }
            }
            Some(Outcome::CharacterCreated(created)) => {
                self.character_initialized = true;
                self.content_revision = Some(created.content_revision);
            }
            Some(Outcome::WorldJoined(joined)) => {
                if self.content_revision.as_ref() != Some(&joined.content_revision) {
                    self.catalog = None;
                }
                self.content_revision = Some(joined.content_revision);
                self.manifest_path = Some(joined.content_manifest_path);
                self.session = Some(joined.world_session_id);
                if self
                    .uncertain_sequence
                    .is_some_and(|sequence| joined.next_sequence > sequence)
                {
                    self.uncertain_sequence = None;
                    self.uncertain_request = None;
                }
                if let Some(snapshot) = joined.snapshot {
                    for event in snapshot.events {
                        self.remember_event(event.event_id);
                    }
                }
            }
            Some(Outcome::LoggedIn(_)) => {
                self.clear_private_world();
            }
            Some(Outcome::WorldLeft(_)) => {
                self.session = None;
                self.uncertain_sequence = None;
                self.uncertain_request = None;
                self.messages.clear();
            }
            Some(Outcome::LoggedOut(_)) => {
                self.clear_private_world();
            }
            Some(Outcome::ActionResult(_)) => {
                self.uncertain_sequence = None;
                self.uncertain_request = None;
            }
            _ => {}
        }
        let mut audio = Vec::new();
        for event in events {
            if let ClientEvent::Gameplay(event) = event
                && self.remember_event(event.event_id.clone())
            {
                if !event.text.is_empty() {
                    self.messages.push(json!({
                        "id":event.event_id,"text":event.text,"channel":"game",
                    }));
                    if self.messages.len() > 128 {
                        self.messages.remove(0);
                    }
                }
                if let Some(event) = view::audio(&event) {
                    audio.push(event);
                }
            }
        }
        self.state_with_details(audio, quote_result, quote_error)
    }

    fn remember_event(&mut self, id: String) -> bool {
        if !self.seen_events.insert(id.clone()) {
            return false;
        }
        self.event_order.push_back(id);
        while self.event_order.len() > 1024 {
            if let Some(id) = self.event_order.pop_front() {
                self.seen_events.remove(&id);
            }
        }
        true
    }

    fn clear_private_world(&mut self) {
        self.session = None;
        self.uncertain_sequence = None;
        self.uncertain_request = None;
        self.character_initialized = false;
        self.content_revision = None;
        self.manifest_path = None;
        self.catalog = None;
        self.messages.clear();
        self.seen_events.clear();
        self.event_order.clear();
        self.pending_quotes.clear();
        self.pending_lifecycle = None;
    }

    pub fn state(&self) -> Result<String, BridgeError> {
        self.state_with_events(Vec::new())
    }

    fn state_with_events(&self, audio: Vec<Value>) -> Result<String, BridgeError> {
        self.state_with_details(audio, None, None)
    }

    fn state_with_details(
        &self,
        audio: Vec<Value>,
        quote: Option<Value>,
        quote_error: Option<&str>,
    ) -> Result<String, BridgeError> {
        let world = match (self.core.snapshot(), self.catalog.as_ref()) {
            (Some(snapshot), Some(catalog)) => {
                Some(view::world(snapshot, catalog, &self.messages)?)
            }
            _ => None,
        };
        let phase = match self.core.phase() {
            Phase::Disconnected => "disconnected",
            Phase::Connecting => "connecting",
            Phase::SigningIn => "signing_in",
            Phase::AccountReady => "account_ready",
            Phase::JoiningWorld => "joining_world",
            Phase::InWorld => "in_world",
            Phase::Reconnecting => "reconnecting",
        };
        Ok(json!({
            "version":1,"phase":phase,
            "authenticated":self.core.authorization_token().is_some(),
            "accountName":self.core.account().map(|account| &account.login_name),
            "characterInitialized":self.character_initialized,
            "gameplayAvailable":self.gameplay_available,"unavailableReason":self.unavailable_reason,
            "capabilities":self.capabilities,"gameplayUiWireSupported":gameplay_ui::WIRE_SUPPORTED,
            "serverBuild":self.server_build,"contentRevision":self.content_revision,
            "contentManifestPath":self.manifest_path,"world":world,"events":audio,
            "worldJoined":self.session.is_some(),
            "quote":quote,"quoteError":quote_error,
            "nextSequence":self.core.next_sequence().map(|value| value.to_string()),
            "uncertainInput":self.uncertain_sequence.is_some(),
        })
        .to_string())
    }
}

fn public_content_path(path: &str) -> bool {
    path.starts_with("/content/")
        && path.len() <= 256
        && path[1..].split('/').all(|segment| {
            !segment.is_empty()
                && !segment.starts_with('.')
                && segment
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.'))
        })
}

/// The sole JS/WASM protocol boundary. No credentials or leases occur in state().
#[wasm_bindgen]
#[derive(Default)]
pub struct BrowserClient {
    inner: Bridge,
}

#[wasm_bindgen]
impl BrowserClient {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self::default()
    }
    pub fn prepare(
        &mut self,
        request_id: &str,
        operation: &str,
        input: &str,
    ) -> Result<Vec<u8>, JsValue> {
        self.inner
            .prepare(request_id, operation, input)
            .map_err(BridgeError::js)
    }
    pub fn submit(&mut self, request_id: &str, input: &str) -> Result<Vec<u8>, JsValue> {
        self.inner
            .submit(request_id, input)
            .map_err(BridgeError::js)
    }
    pub fn submit_selected(
        &mut self,
        request_id: &str,
        input: &str,
        item_id: &str,
    ) -> Result<Vec<u8>, JsValue> {
        self.inner
            .submit_selected(request_id, input, item_id)
            .map_err(BridgeError::js)
    }
    pub fn retry_uncertain_input(&mut self) -> Result<Option<Vec<u8>>, JsValue> {
        self.inner.retry().map_err(BridgeError::js)
    }
    pub fn retry_lifecycle(&mut self) -> Result<Option<Vec<u8>>, JsValue> {
        self.inner.retry_lifecycle().map_err(BridgeError::js)
    }
    pub fn receive(&mut self, bytes: &[u8]) -> Result<String, JsValue> {
        self.inner.receive(bytes).map_err(BridgeError::js)
    }
    pub fn receive_for(&mut self, request: &str, bytes: &[u8]) -> Result<String, JsValue> {
        self.inner
            .receive_for(request, bytes)
            .map_err(BridgeError::js)
    }
    pub fn request_id(&self, bytes: &[u8]) -> Result<String, JsValue> {
        request_id(bytes).map_err(BridgeError::js)
    }
    pub fn request_is_shop_buy(&self, bytes: &[u8]) -> Result<bool, JsValue> {
        let message = ClientMessage::decode(bytes)
            .map_err(|_| BridgeError::protocol("Invalid encoded request.").js())?;
        Ok(matches!(
            message.command,
            Some(Command::WorldInput(game::WorldInput {
                action: Some(game::world_input::Action::ShopBuy(_)),
                ..
            }))
        ))
    }
    pub fn state(&self) -> Result<String, JsValue> {
        self.inner.state().map_err(BridgeError::js)
    }
    pub fn set_catalog(&mut self, input: &str) -> Result<String, JsValue> {
        self.inner.set_catalog(input).map_err(BridgeError::js)
    }
    pub fn transport_lost(&mut self) {
        self.inner.transport_lost();
    }
    /// Transport-only, memory-only. Never copy into UI state, storage, logs or URLs.
    pub fn authorization(&self) -> Option<String> {
        self.inner.authorization().map(str::to_owned)
    }
}

pub fn request_id(bytes: &[u8]) -> Result<String, BridgeError> {
    ClientMessage::decode(bytes)
        .map(|message| message.request_id)
        .map_err(|_| BridgeError::protocol("Invalid encoded request."))
}

pub mod catalog;
mod intent;
mod view;

use std::collections::{BTreeSet, VecDeque};

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
    fn unsupported(message: &str) -> Self {
        Self::new("unsupported", message)
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

#[derive(Default)]
pub struct Bridge {
    core: ClientCore,
    session: Option<String>,
    character_initialized: bool,
    content_revision: Option<String>,
    manifest_path: Option<String>,
    gameplay_available: bool,
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
}

impl Bridge {
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
            "poll" => Command::PollWorld(game::PollWorld {
                world_session_id: self.session.clone().ok_or_else(|| {
                    BridgeError::new("state", "Join the world before requesting updates.")
                })?,
                after_revision: self.core.snapshot().map_or(0, |snapshot| snapshot.revision),
            }),
            "leave" => Command::LeaveWorld(game::LeaveWorld {
                world_session_id: self.session.clone().ok_or_else(|| {
                    BridgeError::new("state", "There is no joined world to leave.")
                })?,
            }),
            _ => return Err(BridgeError::input("Unsupported bridge operation.")),
        };
        self.core
            .prepare(request_id, command)
            .map(|message| message.encode_to_vec())
            .map_err(Into::into)
    }

    pub fn submit(&mut self, request_id: &str, input: &str) -> Result<Vec<u8>, BridgeError> {
        let action = intent::action(input)?;
        let message = self.core.submit_action(request_id, action)?;
        if let Some(Command::WorldInput(input)) = &message.command {
            self.uncertain_sequence = Some(input.sequence);
            self.uncertain_request = Some(request_id.to_owned());
        }
        Ok(message.encode_to_vec())
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

    pub fn transport_lost(&mut self) {
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
            Some(Outcome::WorldSnapshot(snapshot)) => Some(snapshot),
            Some(Outcome::ActionResult(result)) => result.snapshot.as_ref(),
            _ => None,
        };
        if let (Some(snapshot), Some(catalog)) = (snapshot, self.catalog.as_ref()) {
            view::world(snapshot, catalog, &self.messages)?;
        }
        let events = match self.core.handle_response(message) {
            Ok(events) => events,
            Err(error) => {
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
        self.state_with_events(audio)
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
    }

    pub fn state(&self) -> Result<String, BridgeError> {
        self.state_with_events(Vec::new())
    }

    fn state_with_events(&self, audio: Vec<Value>) -> Result<String, BridgeError> {
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
            "serverBuild":self.server_build,"contentRevision":self.content_revision,
            "contentManifestPath":self.manifest_path,"world":world,"events":audio,
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
    pub fn retry_uncertain_input(&mut self) -> Result<Option<Vec<u8>>, JsValue> {
        self.inner.retry().map_err(BridgeError::js)
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

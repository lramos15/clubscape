//! Transactional, server-side dynamic-world persistence; not a gameplay RPC implementation.
//!
//! [`GameStore`] uses the account service's five-second database operation boundary and owned
//! cancellation/discard behavior. Callers supply validated source definitions and bounded,
//! deterministic, nonblocking mechanics; no method accepts client-provided character JSON.
//! See the server README for lock order, retry semantics and the small-population adapter limit.

mod codec;
mod repository;

use std::{collections::BTreeMap, fmt, time::Duration};

use axum::http::StatusCode;
use clubscape_game_types::{
    ActorId, CharacterState, GameEvent, GameIntent, InitialStateDefinition, WorldState,
};
use clubscape_protocol::ErrorCode;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{crypto::token_digest, error::ApiError};

pub use repository::GameStore;

/// Infrastructure ceilings, not source logout/disconnect or combat timers.
pub const MAX_SESSION_LEASE: Duration = Duration::from_secs(300);
pub const MAX_WORLD_LEASE: Duration = Duration::from_secs(60);

/// A sanitized error suitable for an eventual transport adapter. Source diagnostics are logged
/// with this same `error_id`; callback error text, SQL, credentials and state are never exposed.
#[derive(Debug, thiserror::Error)]
#[error("{message} (error ID {error_id})")]
pub struct GameStorageError {
    pub status: StatusCode,
    pub code: ErrorCode,
    pub message: &'static str,
    pub error_id: Uuid,
    pub retry_after_seconds: u32,
}

impl From<ApiError> for GameStorageError {
    fn from(error: ApiError) -> Self {
        tracing::warn!(
            event = "game_storage_failure",
            error_id = %error.error_id,
            status = error.status.as_u16(),
            code = ?error.code,
            "game storage operation failed"
        );
        Self {
            status: error.status,
            code: error.code,
            message: error.message,
            error_id: error.error_id,
            retry_after_seconds: error.retry_after_seconds,
        }
    }
}

/// A digest of an actual account-service token, not a claim that authentication already passed.
/// Every protected operation verifies the live account session inside its transaction.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct AuthTokenDigest([u8; 32]);

impl AuthTokenDigest {
    /// Reuses the account service's canonical token validation and SHA-256 calculation.
    pub fn from_token(token: &str) -> Result<Self, GameStorageError> {
        token_digest(token)
            .map(Self)
            .ok_or_else(|| ApiError::unauthenticated().into())
    }

    /// For a server adapter that already parsed the Authorization header with the account code.
    pub fn from_digest(digest: [u8; 32]) -> Self {
        Self(digest)
    }
}

impl fmt::Debug for AuthTokenDigest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("AuthTokenDigest([redacted])")
    }
}

/// A server-selected definition after content/reference/appearance validation. The repository
/// additionally checks storage shape and bounds; it cannot establish source fidelity itself.
#[derive(Clone, Debug)]
pub struct SourceCharacter {
    pub content_revision: String,
    pub initial_state: InitialStateDefinition,
    pub appearance: BTreeMap<String, u32>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CharacterSnapshot {
    pub account_id: Uuid,
    pub world_id: Uuid,
    pub revision: u64,
    pub last_sequence: u64,
    pub state: CharacterState,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorldSnapshot {
    pub world_id: Uuid,
    pub state: WorldState,
    /// Only the latest server tick receipt is retained here; player command receipts are durable
    /// for all sequences in `processed_game_commands`.
    pub last_tick: Option<TickReceipt>,
}

/// A capability returned only to the server-side repository instance that acquired it.
/// Clones of that repository share its owner; a new repository gets a new owner UUID.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorldLease {
    pub world_id: Uuid,
    pub fence: u64,
    pub expires_at_unix_ms: i64,
    owner_id: Uuid,
}

impl WorldLease {
    pub fn owner_id(&self) -> Uuid {
        self.owner_id
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GameSession {
    pub session_id: Uuid,
    pub account_id: Uuid,
    pub actor_id: ActorId,
    pub world_id: Uuid,
    pub heartbeat_at_unix_ms: i64,
    pub expires_at_unix_ms: i64,
}

impl GameSession {
    pub fn access(&self, authentication: AuthTokenDigest) -> SessionAccess {
        SessionAccess {
            world_id: self.world_id,
            actor_id: self.actor_id.clone(),
            session_id: self.session_id,
            authentication,
        }
    }
}

/// Untrusted identity selectors are checked against the token's account, character and lease.
/// This type deliberately does not accept an account ID.
#[derive(Clone, Debug)]
pub struct SessionAccess {
    pub world_id: Uuid,
    pub actor_id: ActorId,
    pub session_id: Uuid,
    pub authentication: AuthTokenDigest,
}

#[derive(Clone, Debug)]
pub struct GameCommand {
    pub operation_id: Uuid,
    /// Starts at one; a rejected transaction does not consume a sequence.
    pub sequence: u64,
    pub intent: GameIntent,
}

/// The exact committed response, independent of auth/session/lease identifiers. A duplicate
/// returns this historical state/result, not a newly synthesized response from the latest world.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommandReceipt {
    pub operation_id: Uuid,
    pub sequence: u64,
    pub world_id: Uuid,
    pub world_revision: u64,
    pub world_tick: u64,
    pub character_revision: u64,
    pub character: CharacterState,
    pub events: Vec<GameEvent>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub routed_events: Vec<CommittedActorEvent>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommandCommit {
    pub receipt: CommandReceipt,
    pub duplicate: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TickReceipt {
    pub world_id: Uuid,
    pub world_revision: u64,
    pub tick: u64,
    pub events: Vec<GameEvent>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub routed_events: Vec<CommittedActorEvent>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TickCommit {
    pub receipt: TickReceipt,
    pub duplicate: bool,
}

/// Routing and identity are committed with the effect, never inferred from an event's target.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommittedActorEvent {
    pub actor_id: ActorId,
    pub event_id: String,
    pub event: GameEvent,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RoutedCommandCommit {
    pub commit: CommandCommit,
    /// The transaction's current world, including when the receipt is historical.
    pub snapshot: WorldSnapshot,
    pub character_revision: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RoutedTickCommit {
    pub commit: TickCommit,
    pub snapshot: WorldSnapshot,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SessionSnapshot {
    pub session: GameSession,
    pub character: CharacterSnapshot,
    pub world: WorldSnapshot,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum LiveSessionAction {
    Join,
    Leave { session_id: Uuid },
    Logout,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LifecycleReceipt {
    pub operation_id: Uuid,
    pub account_id: Uuid,
    pub world_id: Uuid,
    pub action: LiveSessionAction,
    pub actor_id: Option<ActorId>,
    pub session: Option<GameSession>,
    pub world_revision: u64,
    pub events: Vec<CommittedActorEvent>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LifecycleCommit {
    pub receipt: LifecycleReceipt,
    pub snapshot: WorldSnapshot,
    pub character: Option<CharacterSnapshot>,
    pub duplicate: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorldControlCommit {
    pub snapshot: WorldSnapshot,
    pub events: Vec<CommittedActorEvent>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SessionLoss {
    AuthenticationRevoked,
    TransportLost,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct VerifiedConnections {
    pub connected: std::collections::BTreeSet<ActorId>,
    pub lost: BTreeMap<ActorId, SessionLoss>,
}

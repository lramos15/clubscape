use std::time::Duration;

mod lifecycle;

use clubscape_game_types::{
    Activity, ActorId, CharacterState, GAME_SCHEMA_VERSION, GameEvent, GameIntent, GameResult,
    WorldState,
};
use clubscape_world_engine::ActorEvent;
use sha2::{Digest, Sha256};
use sqlx::{Connection as _, PgConnection, PgPool};
use uuid::Uuid;

use super::{
    AuthTokenDigest, CharacterSnapshot, CommandCommit, CommandReceipt, CommittedActorEvent,
    GameCommand, GameSession, GameStorageError, MAX_SESSION_LEASE, MAX_WORLD_LEASE,
    RoutedCommandCommit, RoutedTickCommit, SessionAccess, SessionSnapshot, SourceCharacter,
    TickCommit, TickReceipt, WorldLease, WorldSnapshot,
    codec::{
        self, MAX_CHARACTERS, MAX_RESULT_BYTES, MAX_WORLD_BYTES, callback_error, conflict, decode,
        encode, increment, intent_hash, number, validate_character, validate_events,
        validate_world,
    },
};
use crate::{MIGRATIONS, database, error::ApiError};

/// One small-population dynamic-world snapshot is the state authority. Static content is supplied
/// separately by the runtime. Every operation, including commit and pool release, reuses the
/// account service's cancellation-owned five-second database boundary.
#[derive(Clone)]
pub struct GameStore {
    pool: PgPool,
    owner_id: Uuid,
}

impl GameStore {
    /// The caller owns pool configuration. Clones share an owner UUID; a fresh instance must acquire
    /// its own world lease (and cannot reuse a previous process's fencing capability).
    pub fn new(pool: PgPool) -> Self {
        Self {
            pool,
            owner_id: Uuid::new_v4(),
        }
    }

    /// Applies the same embedded account/game migrations used by `Service::bind`.
    pub async fn migrate(&self) -> Result<(), GameStorageError> {
        database::run(&self.pool, "game_migrations", true, |connection| {
            Box::pin(async move {
                MIGRATIONS
                    .run_direct(connection)
                    .await
                    .map_err(|error| match error {
                        sqlx::migrate::MigrateError::Execute(source)
                        | sqlx::migrate::MigrateError::ExecuteMigration(source, _) => {
                            ApiError::database(source)
                        }
                        _ => ApiError::internal("game_migration_validation"),
                    })
            })
        })
        .await
        .map_err(Into::into)
    }

    /// Closes this pool, including all its clones, using the account service's two-second bound.
    pub async fn close(self) -> Result<(), GameStorageError> {
        if database::close_pool(self.pool).await {
            Ok(())
        } else {
            Err(ApiError::deadline(
                "game_storage_shutdown",
                "database_cleanup",
                false,
                database::CLEANUP_TIMEOUT,
            )
            .into())
        }
    }

    /// Atomically installs a source-derived, character-free world at tick/revision zero. Repeated
    /// initialization of the same content/schema returns the stored world, never resets it.
    pub async fn initialize_world(
        &self,
        world_id: Uuid,
        initial: WorldState,
    ) -> Result<WorldSnapshot, GameStorageError> {
        codec::uuid(world_id)?;
        validate_world(&initial)?;
        if !initial.characters.is_empty() || initial.tick != 0 || initial.revision != 0 {
            return Err(ApiError::invalid(
                "World initialization requires no characters and tick/revision zero.",
            )
            .into());
        }
        let json = encode(&initial, MAX_WORLD_BYTES)?;
        database::run(
            &self.pool,
            "game_initialize_world",
            true,
            move |connection| {
                Box::pin(async move {
                    let mut transaction = connection.begin().await.map_err(ApiError::database)?;
                    sqlx::query(
                        "INSERT INTO game_worlds
                         (world_id, content_revision, schema_version, state, revision, tick)
                     VALUES ($1, $2, $3, $4::jsonb, 0, 0) ON CONFLICT (world_id) DO NOTHING",
                    )
                    .bind(world_id)
                    .bind(&initial.content_revision)
                    .bind(GAME_SCHEMA_VERSION as i32)
                    .bind(json)
                    .execute(&mut *transaction)
                    .await
                    .map_err(ApiError::database)?;
                    let stored = lock_world(&mut transaction, world_id).await?;
                    if stored.state.content_revision != initial.content_revision
                        || stored.state.schema_version != initial.schema_version
                    {
                        return Err(conflict(
                            "The world already uses a different content revision or schema.",
                        ));
                    }
                    lock_characters(&mut transaction, &stored).await?;
                    transaction.commit().await.map_err(ApiError::database)?;
                    Ok(stored)
                })
            },
        )
        .await
        .map_err(Into::into)
    }

    /// A server-only complete dynamic snapshot, not a client broadcast payload.
    pub async fn load_world(&self, world_id: Uuid) -> Result<WorldSnapshot, GameStorageError> {
        codec::uuid(world_id)?;
        database::run(&self.pool, "game_load_world", false, move |connection| {
            Box::pin(async move {
                let mut transaction = connection.begin().await.map_err(ApiError::database)?;
                let stored = lock_world(&mut transaction, world_id).await?;
                lock_characters(&mut transaction, &stored).await?;
                transaction.commit().await.map_err(ApiError::database)?;
                Ok(stored)
            })
        })
        .await
        .map_err(Into::into)
    }

    /// Reconnects an unexpired lease owned by this repository, or acquires a new fence after
    /// release/expiry. A different unexpired owner is always rejected.
    pub async fn acquire_world_lease(
        &self,
        world_id: Uuid,
        duration: Duration,
    ) -> Result<WorldLease, GameStorageError> {
        codec::uuid(world_id)?;
        let milliseconds = lease_milliseconds(duration, MAX_WORLD_LEASE)?;
        let owner = self.owner_id;
        database::run(&self.pool, "game_acquire_world_lease", true, move |connection| {
            Box::pin(async move {
                let mut transaction = connection.begin().await.map_err(ApiError::database)?;
                lock_world(&mut transaction, world_id).await?;
                let (fence, expires): (i64, i64) = sqlx::query_as(
                    "WITH stamp AS MATERIALIZED (SELECT clock_timestamp() AS now)
                     UPDATE game_worlds w SET
                         lease_fence = CASE WHEN lease_owner = $2 AND lease_expires_at > stamp.now
                                            THEN lease_fence ELSE lease_fence + 1 END,
                         lease_owner = $2,
                         lease_expires_at = stamp.now + $3::bigint * INTERVAL '1 millisecond'
                     FROM stamp WHERE world_id = $1 AND lease_fence < 9223372036854775807
                         AND (lease_owner IS NULL OR lease_owner = $2 OR lease_expires_at <= stamp.now)
                     RETURNING lease_fence, floor(extract(epoch FROM lease_expires_at) * 1000)::bigint",
                )
                .bind(world_id)
                .bind(owner)
                .bind(milliseconds)
                .fetch_optional(&mut *transaction)
                .await
                .map_err(ApiError::database)?
                .ok_or_else(|| conflict("The world has another active owner or an exhausted fence."))?;
                transaction.commit().await.map_err(ApiError::database)?;
                Ok(WorldLease {
                    world_id,
                    owner_id: owner,
                    fence: fence as u64,
                    expires_at_unix_ms: expires,
                })
            })
        })
        .await
        .map_err(Into::into)
    }

    pub async fn renew_world_lease(
        &self,
        lease: &WorldLease,
        duration: Duration,
    ) -> Result<WorldLease, GameStorageError> {
        self.local_lease(lease)?;
        let milliseconds = lease_milliseconds(duration, MAX_WORLD_LEASE)?;
        let mut lease = lease.clone();
        database::run(
            &self.pool,
            "game_renew_world_lease",
            true,
            move |connection| {
                Box::pin(async move {
                    let mut transaction = connection.begin().await.map_err(ApiError::database)?;
                    lock_world(&mut transaction, lease.world_id).await?;
                    lease.expires_at_unix_ms = sqlx::query_scalar(
                        "WITH stamp AS MATERIALIZED (SELECT clock_timestamp() AS now)
                     UPDATE game_worlds w SET
                         lease_expires_at = stamp.now + $4::bigint * INTERVAL '1 millisecond'
                     FROM stamp WHERE world_id = $1 AND lease_owner = $2 AND lease_fence = $3
                         AND lease_expires_at > stamp.now
                     RETURNING floor(extract(epoch FROM lease_expires_at) * 1000)::bigint",
                    )
                    .bind(lease.world_id)
                    .bind(lease.owner_id)
                    .bind(number(lease.fence)?)
                    .bind(milliseconds)
                    .fetch_optional(&mut *transaction)
                    .await
                    .map_err(ApiError::database)?
                    .ok_or_else(stale_fence)?;
                    transaction.commit().await.map_err(ApiError::database)?;
                    Ok(lease)
                })
            },
        )
        .await
        .map_err(Into::into)
    }

    pub async fn release_world_lease(&self, lease: &WorldLease) -> Result<(), GameStorageError> {
        self.local_lease(lease)?;
        let lease = lease.clone();
        database::run(
            &self.pool,
            "game_release_world_lease",
            true,
            move |connection| {
                Box::pin(async move {
                    let mut transaction = connection.begin().await.map_err(ApiError::database)?;
                    lock_world(&mut transaction, lease.world_id).await?;
                    let changed = sqlx::query(
                        "UPDATE game_worlds SET lease_owner = NULL, lease_expires_at = NULL
                     WHERE world_id = $1 AND lease_owner = $2 AND lease_fence = $3
                         AND lease_expires_at > clock_timestamp()",
                    )
                    .bind(lease.world_id)
                    .bind(lease.owner_id)
                    .bind(number(lease.fence)?)
                    .execute(&mut *transaction)
                    .await
                    .map_err(ApiError::database)?;
                    if changed.rows_affected() != 1 {
                        return Err(stale_fence());
                    }
                    transaction.commit().await.map_err(ApiError::database)?;
                    Ok(())
                })
            },
        )
        .await
        .map_err(Into::into)
    }

    /// Creates once, from all fields of the provided source definition. Login name becomes the
    /// display name; actor ID, idle activity, and sequence bookkeeping are server-owned metadata.
    /// A retry returns the existing character even if a different initial definition is supplied.
    pub async fn create_character(
        &self,
        lease: &WorldLease,
        authentication: AuthTokenDigest,
        definition: SourceCharacter,
    ) -> Result<CharacterSnapshot, GameStorageError> {
        self.create_character_with(lease, authentication, definition, |_| Ok(()))
            .await
    }

    /// Creation-only source runtime initialization; retries never call the initializer again.
    pub async fn create_character_with<F>(
        &self,
        lease: &WorldLease,
        authentication: AuthTokenDigest,
        definition: SourceCharacter,
        initialize: F,
    ) -> Result<CharacterSnapshot, GameStorageError>
    where
        F: FnOnce(&mut CharacterState) -> GameResult<()> + Send + 'static,
    {
        self.local_lease(lease)?;
        let lease = lease.clone();
        database::run(&self.pool, "game_create_character", true, move |connection| {
            Box::pin(async move {
                let mut transaction = connection.begin().await.map_err(ApiError::database)?;
                let mut world = lock_world(&mut transaction, lease.world_id).await?;
                ensure_fence(&mut transaction, &lease).await?;
                let account = lock_account(&mut transaction, authentication).await?;
                let characters = lock_characters(&mut transaction, &world).await?;
                lock_authentication(&mut transaction, account.account_id, authentication).await?;
                if let Some(existing) = characters.iter().find(|row| row.account_id == account.account_id) {
                    let snapshot = character_snapshot(&world, existing)?;
                    transaction.commit().await.map_err(ApiError::database)?;
                    return Ok(snapshot);
                }
                let elsewhere: bool = sqlx::query_scalar(
                    "SELECT EXISTS (SELECT 1 FROM game_characters WHERE account_id = $1)",
                )
                .bind(account.account_id)
                .fetch_one(&mut *transaction)
                .await
                .map_err(ApiError::database)?;
                if elsewhere {
                    return Err(conflict("This account already has a character in another world."));
                }
                if definition.content_revision != world.state.content_revision {
                    return Err(conflict("The character definition does not match the world's content revision."));
                }
                if world.state.characters.len() >= MAX_CHARACTERS {
                    return Err(conflict("This world's character storage capacity has been reached."));
                }
                if definition.initial_state.source.is_empty()
                    || definition.initial_state.source.len() > 128
                    || definition.initial_state.source.iter().any(|source| {
                        !codec::text(&source.reference, 2048) || !codec::text(&source.revision, 256)
                    })
                {
                    return Err(ApiError::invalid("A validated source initial-state definition is required."));
                }
                let initial = definition.initial_state;
                let actor_id = ActorId::new(format!("actor.{}", Uuid::new_v4().simple()))
                    .map_err(|_| ApiError::internal("game_actor_identity"))?;
                let mut character = CharacterState {
                    schema_version: GAME_SCHEMA_VERSION,
                    actor_id: actor_id.clone(),
                    display_name: account.login_name,
                    appearance: definition.appearance,
                    region: initial.region,
                    tile: initial.tile,
                    inventory: initial.inventory,
                    equipment: initial.equipment,
                    bank: initial.bank,
                    skills: initial.skills,
                    hitpoints: initial.hitpoints,
                    prayer_points: initial.prayer_points,
                    run_energy: initial.run_energy,
                    quest_points: initial.quest_points,
                    tutorial_stage: initial.tutorial_stage,
                    quests: initial.quests,
                    flags: initial.flags,
                    interfaces: initial.interfaces,
                    activity: Activity::Idle,
                    dialogue: None,
                    last_action_tick: world.state.tick,
                    last_command_sequence: 0,
                    runtime: clubscape_game_types::CharacterRuntime::from_initial_definition(&initial.runtime),
                };
                initialize(&mut character).map_err(callback_error)?;
                if character.actor_id != actor_id || character.last_command_sequence != 0
                    || character.last_action_tick != world.state.tick
                {
                    return Err(ApiError::internal("game_character_initializer_metadata"));
                }
                validate_character(&character, world.state.tick)?;
                let previous_revision = world.state.revision;
                world.state.revision = increment(previous_revision)?;
                world.state.characters.insert(actor_id.clone(), character.clone());
                validate_world(&world.state)?;
                let json = encode(&world.state, MAX_WORLD_BYTES)?;
                validate_authentication(&mut transaction, account.account_id, authentication).await?;
                save_world(&mut transaction, &lease, &world.state, previous_revision, json, None).await?;
                sqlx::query(
                    "INSERT INTO game_characters (account_id, actor_id, world_id, revision, last_sequence)
                     VALUES ($1, $2, $3, 1, 0)",
                )
                .bind(account.account_id)
                .bind(actor_id.as_str())
                .bind(lease.world_id)
                .execute(&mut *transaction)
                .await
                .map_err(ApiError::database)?;
                validate_authentication(&mut transaction, account.account_id, authentication).await?;
                ensure_fence(&mut transaction, &lease).await?;
                transaction.commit().await.map_err(ApiError::database)?;
                Ok(CharacterSnapshot {
                    account_id: account.account_id,
                    world_id: lease.world_id,
                    revision: 1,
                    last_sequence: 0,
                    state: character,
                })
            })
        })
        .await
        .map_err(Into::into)
    }

    /// Reads only the authenticated account's character. No character is created by this read.
    pub async fn load_character(
        &self,
        world_id: Uuid,
        authentication: AuthTokenDigest,
    ) -> Result<Option<CharacterSnapshot>, GameStorageError> {
        codec::uuid(world_id)?;
        database::run(
            &self.pool,
            "game_load_character",
            false,
            move |connection| {
                Box::pin(async move {
                    let mut transaction = connection.begin().await.map_err(ApiError::database)?;
                    let world = lock_world(&mut transaction, world_id).await?;
                    let account = lock_account(&mut transaction, authentication).await?;
                    let characters = lock_characters(&mut transaction, &world).await?;
                    lock_authentication(&mut transaction, account.account_id, authentication)
                        .await?;
                    let snapshot = characters
                        .iter()
                        .find(|row| row.account_id == account.account_id)
                        .map(|row| character_snapshot(&world, row))
                        .transpose()?;
                    transaction.commit().await.map_err(ApiError::database)?;
                    Ok(snapshot)
                })
            },
        )
        .await
        .map_err(Into::into)
    }

    /// Reuses the same live token's session ID. A different live auth token cannot take an active
    /// lease; explicit leave, player-lease expiry, or old auth revocation/expiry enables takeover.
    pub async fn join_session(
        &self,
        world_id: Uuid,
        actor_id: ActorId,
        authentication: AuthTokenDigest,
        duration: Duration,
    ) -> Result<GameSession, GameStorageError> {
        codec::uuid(world_id)?;
        let milliseconds = lease_milliseconds(duration, MAX_SESSION_LEASE)?;
        database::run(&self.pool, "game_join_session", true, move |connection| {
            Box::pin(async move {
                let mut transaction = connection.begin().await.map_err(ApiError::database)?;
                let world = lock_world(&mut transaction, world_id).await?;
                let account = lock_account(&mut transaction, authentication).await?;
                let characters = lock_characters(&mut transaction, &world).await?;
                lock_authentication(&mut transaction, account.account_id, authentication).await?;
                owned_character(&characters, account.account_id, &actor_id)?;
                let existing = lock_session(&mut transaction, account.account_id).await?;
                let mut session_id = Uuid::new_v4();
                if let Some(existing) = existing
                    && session_is_live(&mut transaction, existing.session_id).await?
                {
                    if existing.token_digest == authentication.0 {
                        session_id = existing.session_id;
                    } else {
                        let owner_live: bool = sqlx::query_scalar(
                            "SELECT EXISTS (SELECT 1 FROM account_sessions
                             WHERE account_id = $1 AND token_digest = $2
                                 AND expires_at > clock_timestamp())",
                        )
                        .bind(account.account_id)
                        .bind(&existing.token_digest)
                        .fetch_one(&mut *transaction)
                        .await
                        .map_err(ApiError::database)?;
                        if owner_live {
                            return Err(conflict(
                                "Another authentication session owns the active game lease.",
                            ));
                        }
                    }
                }
                let session = put_session(
                    &mut transaction,
                    &SessionAccess {
                        world_id,
                        actor_id,
                        session_id,
                        authentication,
                    },
                    account.account_id,
                    milliseconds,
                )
                .await?;
                validate_authentication(&mut transaction, account.account_id, authentication)
                    .await?;
                transaction.commit().await.map_err(ApiError::database)?;
                session.public()
            })
        })
        .await
        .map_err(Into::into)
    }

    pub async fn read_session(
        &self,
        access: &SessionAccess,
    ) -> Result<GameSession, GameStorageError> {
        self.session_snapshot(access, None)
            .await
            .map(|snapshot| snapshot.session)
    }

    /// Heartbeats are capped at the actual auth token's expiry and never revive an expired lease.
    pub async fn heartbeat_session(
        &self,
        access: &SessionAccess,
        duration: Duration,
    ) -> Result<GameSession, GameStorageError> {
        self.session_snapshot(access, Some(duration))
            .await
            .map(|snapshot| snapshot.session)
    }

    /// Authenticated view data and optional heartbeat share the same locked transaction.
    pub async fn session_snapshot(
        &self,
        access: &SessionAccess,
        duration: Option<Duration>,
    ) -> Result<SessionSnapshot, GameStorageError> {
        validate_access(access)?;
        let milliseconds = duration
            .map(|duration| lease_milliseconds(duration, MAX_SESSION_LEASE))
            .transpose()?;
        let access = access.clone();
        database::run(
            &self.pool,
            if milliseconds.is_some() {
                "game_heartbeat_session"
            } else {
                "game_read_session"
            },
            milliseconds.is_some(),
            move |connection| {
                Box::pin(async move {
                    let mut transaction = connection.begin().await.map_err(ApiError::database)?;
                    let world = lock_world(&mut transaction, access.world_id).await?;
                    let player = lock_player(&mut transaction, &world, &access).await?;
                    let character = character_snapshot(
                        &world,
                        owned_character(
                            &player.characters,
                            player.account.account_id,
                            &access.actor_id,
                        )?,
                    )?;
                    let session = if let Some(milliseconds) = milliseconds {
                        put_session(
                            &mut transaction,
                            &access,
                            player.account.account_id,
                            milliseconds,
                        )
                        .await?
                    } else {
                        player.session
                    };
                    validate_player(&mut transaction, &access, player.account.account_id).await?;
                    transaction.commit().await.map_err(ApiError::database)?;
                    Ok(SessionSnapshot {
                        session: session.public()?,
                        character,
                        world,
                    })
                })
            },
        )
        .await
        .map_err(Into::into)
    }

    /// Releases infrastructure ownership only. The runtime must implement source logout/combat/
    /// disconnect mechanics separately. Repeating leave on an absent row returns `false`.
    pub async fn leave_session(&self, access: &SessionAccess) -> Result<bool, GameStorageError> {
        validate_access(access)?;
        let access = access.clone();
        database::run(&self.pool, "game_leave_session", true, move |connection| {
            Box::pin(async move {
                let mut transaction = connection.begin().await.map_err(ApiError::database)?;
                let world = lock_world(&mut transaction, access.world_id).await?;
                let account = lock_account(&mut transaction, access.authentication).await?;
                let characters = lock_characters(&mut transaction, &world).await?;
                lock_authentication(&mut transaction, account.account_id, access.authentication)
                    .await?;
                owned_character(&characters, account.account_id, &access.actor_id)?;
                let existing = lock_session(&mut transaction, account.account_id).await?;
                let changed = if let Some(existing) = existing {
                    existing.matches(&access)?;
                    sqlx::query(
                        "DELETE FROM game_sessions WHERE account_id = $1 AND session_id = $2",
                    )
                    .bind(account.account_id)
                    .bind(access.session_id)
                    .execute(&mut *transaction)
                    .await
                    .map_err(ApiError::database)?
                    .rows_affected()
                        == 1
                } else {
                    false
                };
                validate_authentication(
                    &mut transaction,
                    account.account_id,
                    access.authentication,
                )
                .await?;
                transaction.commit().await.map_err(ApiError::database)?;
                Ok(changed)
            })
        })
        .await
        .map_err(Into::into)
    }

    /// Applies trusted deterministic mechanics once, under live auth/player/world ownership.
    /// The callback may change any existing character and dynamic entities, but not identities,
    /// schema/content, world tick/revision or command sequences. It must perform no I/O or external
    /// side effects. The runtime, not this journal, enforces source action/tick eligibility.
    pub async fn commit_command<F>(
        &self,
        lease: &WorldLease,
        access: &SessionAccess,
        command: GameCommand,
        apply: F,
    ) -> Result<CommandCommit, GameStorageError>
    where
        F: FnOnce(&mut WorldState, &ActorId, &GameIntent) -> GameResult<Vec<GameEvent>>
            + Send
            + 'static,
    {
        self.commit_command_inner(lease, access, command, None, move |world, actor, intent| {
            apply(world, actor, intent).map(EventOutput::Legacy)
        })
        .await
        .map(|outcome| outcome.commit)
    }

    /// An observed revision may be older than a source tick; a future observation is rejected.
    /// Sequences and intent hashes remain strict. Replays never invoke the routed callback.
    pub async fn commit_routed_command<F>(
        &self,
        lease: &WorldLease,
        access: &SessionAccess,
        command: GameCommand,
        observed_revision: Option<u64>,
        apply: F,
    ) -> Result<RoutedCommandCommit, GameStorageError>
    where
        F: FnOnce(&mut WorldState, &ActorId, &GameIntent) -> GameResult<Vec<ActorEvent>>
            + Send
            + 'static,
    {
        self.commit_command_inner(
            lease,
            access,
            command,
            observed_revision,
            move |world, actor, intent| apply(world, actor, intent).map(EventOutput::Routed),
        )
        .await
    }

    async fn commit_command_inner<F>(
        &self,
        lease: &WorldLease,
        access: &SessionAccess,
        command: GameCommand,
        observed_revision: Option<u64>,
        apply: F,
    ) -> Result<RoutedCommandCommit, GameStorageError>
    where
        F: FnOnce(&mut WorldState, &ActorId, &GameIntent) -> GameResult<EventOutput>
            + Send
            + 'static,
    {
        self.local_lease(lease)?;
        validate_access(access)?;
        codec::uuid(command.operation_id)?;
        number(command.sequence)?;
        if command.sequence == 0 || access.world_id != lease.world_id {
            return Err(ApiError::invalid(
                "The command requires a matching world and a positive sequence.",
            )
            .into());
        }
        let hash = intent_hash(&command.intent)?;
        let lease = lease.clone();
        let access = access.clone();
        database::run(&self.pool, "game_commit_command", true, move |connection| {
            Box::pin(async move {
                let mut transaction = connection.begin().await.map_err(ApiError::database)?;
                let mut world = lock_world(&mut transaction, lease.world_id).await?;
                ensure_fence(&mut transaction, &lease).await?;
                let player = lock_player(&mut transaction, &world, &access).await?;
                let character = owned_character(
                    &player.characters,
                    player.account.account_id,
                    &access.actor_id,
                )?;
                if let Some(receipt) =
                    journal_result(&mut transaction, &world, character, &command, hash).await?
                {
                    validate_player(&mut transaction, &access, player.account.account_id).await?;
                    ensure_fence(&mut transaction, &lease).await?;
                    transaction.commit().await.map_err(ApiError::database)?;
                    return Ok(RoutedCommandCommit {
                        commit: CommandCommit {
                            receipt,
                            duplicate: true,
                        },
                        snapshot: world,
                        character_revision: character.revision as u64,
                    });
                }
                if observed_revision.is_some_and(|revision| revision > character.revision as u64) {
                    return Err(conflict(
                        "The observed character revision is ahead of authoritative state.",
                    ));
                }
                let used: bool = sqlx::query_scalar(
                    "SELECT EXISTS (SELECT 1 FROM game_lifecycle_commands WHERE account_id = $1 AND operation_id = $2)",
                ).bind(player.account.account_id).bind(command.operation_id)
                    .fetch_one(&mut *transaction).await.map_err(ApiError::database)?;
                if used { return Err(conflict("The operation ID belongs to a lifecycle operation.")); }
                if command.sequence != increment(character.last_sequence as u64)? {
                    return Err(conflict("The command sequence is stale or has a gap."));
                }
                let previous = world.state.clone();
                let output = apply(&mut world.state, &access.actor_id, &command.intent)
                    .map_err(callback_error)?;
                validate_transition(&previous, &world.state, previous.tick)?;
                world
                    .state
                    .characters
                    .get_mut(&access.actor_id)
                    .ok_or_else(|| ApiError::internal("game_callback_identity"))?
                    .last_command_sequence = command.sequence;
                world.state.revision = increment(previous.revision)?;
                let (events, routed_events) = commit_events(output, &world)?;
                let changed = changed_characters(&previous, &world.state, &player.characters)?;
                let updated =
                    owned_character(&changed, player.account.account_id, &access.actor_id)?;
                let receipt = CommandReceipt {
                    operation_id: command.operation_id,
                    sequence: command.sequence,
                    world_id: lease.world_id,
                    world_revision: world.state.revision,
                    world_tick: world.state.tick,
                    character_revision: updated.revision as u64,
                    character: world.state.characters[&access.actor_id].clone(),
                    events,
                    routed_events,
                };
                let result_json = encode(&receipt, MAX_RESULT_BYTES)?;
                let world_json = encode(&world.state, MAX_WORLD_BYTES)?;
                validate_player(&mut transaction, &access, player.account.account_id).await?;
                save_world(
                    &mut transaction,
                    &lease,
                    &world.state,
                    previous.revision,
                    world_json,
                    None,
                )
                .await?;
                save_characters(&mut transaction, lease.world_id, &changed).await?;
                sqlx::query(
                    "INSERT INTO processed_game_commands
                         (account_id, actor_id, world_id, operation_id, sequence, intent_version,
                          intent_hash, world_revision, character_revision, committed_result)
                     VALUES ($1, $2, $3, $4, $5, 1, $6, $7, $8, $9::jsonb)",
                )
                .bind(player.account.account_id)
                .bind(access.actor_id.as_str())
                .bind(lease.world_id)
                .bind(command.operation_id)
                .bind(number(command.sequence)?)
                .bind(hash.as_slice())
                .bind(number(receipt.world_revision)?)
                .bind(number(receipt.character_revision)?)
                .bind(result_json)
                .execute(&mut *transaction)
                .await
                .map_err(ApiError::database)?;
                validate_player(&mut transaction, &access, player.account.account_id).await?;
                ensure_fence(&mut transaction, &lease).await?;
                transaction.commit().await.map_err(ApiError::database)?;
                Ok(RoutedCommandCommit {
                    character_revision: receipt.character_revision,
                    commit: CommandCommit {
                        receipt,
                        duplicate: false,
                    },
                    snapshot: world,
                })
            })
        })
        .await
        .map_err(Into::into)
    }

    /// Server scheduler boundary only; never expose this method as a client tick command.
    /// The callback sees `expected_tick + 1` and must leave that tick and all sequences unchanged.
    /// The latest tick receipt is retryable; older ticks fail rather than rerunning simulation.
    /// Source cadence (600 ms), randomness and gameplay disconnect rules belong to the runtime.
    pub async fn commit_tick<F>(
        &self,
        lease: &WorldLease,
        expected_tick: u64,
        apply: F,
    ) -> Result<TickCommit, GameStorageError>
    where
        F: FnOnce(&mut WorldState) -> GameResult<Vec<GameEvent>> + Send + 'static,
    {
        self.commit_tick_inner(
            lease,
            expected_tick,
            TickAuthority::Unrestricted,
            move |world, _| apply(world).map(EventOutput::Legacy),
        )
        .await
        .map(|outcome| outcome.commit)
    }

    /// The optional sessions are an infrastructure safety precondition, not a source presence
    /// policy. If supplied, every stored actor must have its exact, still-live authenticated lease.
    pub async fn commit_routed_tick<F>(
        &self,
        lease: &WorldLease,
        expected_tick: u64,
        sessions: Vec<SessionAccess>,
        apply: F,
    ) -> Result<RoutedTickCommit, GameStorageError>
    where
        F: FnOnce(&mut WorldState) -> GameResult<Vec<ActorEvent>> + Send + 'static,
    {
        self.commit_tick_inner(
            lease,
            expected_tick,
            TickAuthority::Strict(sessions),
            move |world, _| apply(world).map(EventOutput::Routed),
        )
        .await
    }

    /// Source lifecycle processing receives verified connection facts, not an all-online set.
    pub async fn commit_live_tick<F>(
        &self,
        lease: &WorldLease,
        expected_tick: u64,
        sessions: Vec<SessionAccess>,
        apply: F,
    ) -> Result<RoutedTickCommit, GameStorageError>
    where
        F: FnOnce(&mut WorldState, &super::VerifiedConnections) -> GameResult<Vec<ActorEvent>>
            + Send
            + 'static,
    {
        self.commit_tick_inner(
            lease,
            expected_tick,
            TickAuthority::Live(sessions),
            move |world, facts| apply(world, facts).map(EventOutput::Routed),
        )
        .await
    }

    async fn commit_tick_inner<F>(
        &self,
        lease: &WorldLease,
        expected_tick: u64,
        authority: TickAuthority,
        apply: F,
    ) -> Result<RoutedTickCommit, GameStorageError>
    where
        F: FnOnce(&mut WorldState, &super::VerifiedConnections) -> GameResult<EventOutput>
            + Send
            + 'static,
    {
        self.local_lease(lease)?;
        let next_tick = increment(expected_tick)?;
        let lease = lease.clone();
        database::run(&self.pool, "game_commit_tick", true, move |connection| {
            Box::pin(async move {
                let mut transaction = connection.begin().await.map_err(ApiError::database)?;
                let mut world = lock_world(&mut transaction, lease.world_id).await?;
                ensure_fence(&mut transaction, &lease).await?;
                let characters = lock_characters(&mut transaction, &world).await?;
                if world.state.tick == next_tick
                    && let Some(receipt) = world.last_tick.clone()
                {
                    ensure_fence(&mut transaction, &lease).await?;
                    transaction.commit().await.map_err(ApiError::database)?;
                    return Ok(RoutedTickCommit {
                        commit: TickCommit {
                            receipt,
                            duplicate: true,
                        },
                        snapshot: world,
                    });
                }
                if world.state.tick != expected_tick {
                    return Err(conflict("The expected world tick is stale or has a gap."));
                }
                let facts = match &authority {
                    TickAuthority::Unrestricted => super::VerifiedConnections::default(),
                    TickAuthority::Strict(sessions) => {
                        lock_tick_sessions(&mut transaction, &world, sessions).await?;
                        super::VerifiedConnections::default()
                    }
                    TickAuthority::Live(sessions) => {
                        lifecycle::connection_facts(&mut transaction, &world, sessions, true)
                            .await?
                    }
                };
                let previous = world.state.clone();
                world.state.tick = next_tick;
                let output = apply(&mut world.state, &facts).map_err(callback_error)?;
                validate_transition(&previous, &world.state, next_tick)?;
                world.state.revision = increment(previous.revision)?;
                let (events, routed_events) = commit_events(output, &world)?;
                let changed = changed_characters(&previous, &world.state, &characters)?;
                let receipt = TickReceipt {
                    world_id: lease.world_id,
                    world_revision: world.state.revision,
                    tick: next_tick,
                    events,
                    routed_events,
                };
                let result_json = encode(&receipt, MAX_RESULT_BYTES)?;
                let world_json = encode(&world.state, MAX_WORLD_BYTES)?;
                save_world(
                    &mut transaction,
                    &lease,
                    &world.state,
                    previous.revision,
                    world_json,
                    Some(result_json),
                )
                .await?;
                save_characters(&mut transaction, lease.world_id, &changed).await?;
                match &authority {
                    TickAuthority::Strict(sessions) => {
                        check_tick_sessions(&mut transaction, &world, sessions).await?
                    }
                    TickAuthority::Live(sessions) => {
                        if lifecycle::connection_facts(&mut transaction, &world, sessions, false)
                            .await?
                            != facts
                        {
                            return Err(conflict(
                                "Tick connection facts changed; retry the same tick.",
                            ));
                        }
                    }
                    TickAuthority::Unrestricted => {}
                }
                ensure_fence(&mut transaction, &lease).await?;
                transaction.commit().await.map_err(ApiError::database)?;
                world.last_tick = Some(receipt.clone());
                Ok(RoutedTickCommit {
                    commit: TickCommit {
                        receipt,
                        duplicate: false,
                    },
                    snapshot: world,
                })
            })
        })
        .await
        .map_err(Into::into)
    }

    /// Pins immutable source identity and a private PRF key. Neither is part of a public world view.
    pub(crate) async fn runtime_key(
        &self,
        lease: &WorldLease,
        artifact_hash: String,
    ) -> Result<[u8; 32], GameStorageError> {
        self.local_lease(lease)?;
        if artifact_hash.len() != 64
            || !artifact_hash
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err(ApiError::invalid("A SHA-256 artifact identity is required.").into());
        }
        let mut candidate = [0; 32];
        getrandom::fill(&mut candidate).map_err(|_| ApiError::internal("game_random_entropy"))?;
        let lease = lease.clone();
        database::run(&self.pool, "game_runtime_identity", true, move |connection| Box::pin(async move {
            let mut transaction = connection.begin().await.map_err(ApiError::database)?;
            lock_world(&mut transaction, lease.world_id).await?;
            ensure_fence(&mut transaction, &lease).await?;
            sqlx::query(
                "UPDATE game_worlds SET runtime_artifact_sha256 = $2, runtime_random_key = $3
                 WHERE world_id = $1 AND runtime_artifact_sha256 IS NULL",
            ).bind(lease.world_id).bind(&artifact_hash).bind(candidate.as_slice())
                .execute(&mut *transaction).await.map_err(ApiError::database)?;
            let (stored_hash, key): (String, Vec<u8>) = sqlx::query_as(
                "SELECT runtime_artifact_sha256, runtime_random_key FROM game_worlds WHERE world_id = $1",
            ).bind(lease.world_id).fetch_one(&mut *transaction).await.map_err(ApiError::database)?;
            if stored_hash != artifact_hash {
                return Err(conflict("This world is pinned to a different compiled artifact."));
            }
            let key = key.try_into().map_err(|_| ApiError::internal("game_random_key_shape"))?;
            ensure_fence(&mut transaction, &lease).await?;
            transaction.commit().await.map_err(ApiError::database)?;
            Ok(key)
        })).await.map_err(Into::into)
    }

    fn local_lease(&self, lease: &WorldLease) -> Result<(), ApiError> {
        codec::uuid(lease.world_id)?;
        if lease.owner_id != self.owner_id || lease.fence == 0 {
            return Err(stale_fence());
        }
        number(lease.fence)?;
        Ok(())
    }
}

#[derive(sqlx::FromRow)]
struct WorldRow {
    world_id: Uuid,
    content_revision: String,
    schema_version: i32,
    revision: i64,
    tick: i64,
    state_json: Option<String>,
    last_tick_json: Option<String>,
    has_tick_result: bool,
}

async fn lock_world(
    connection: &mut PgConnection,
    world_id: Uuid,
) -> Result<WorldSnapshot, ApiError> {
    let row = sqlx::query_as::<_, WorldRow>(
        "SELECT world_id, content_revision, schema_version, revision, tick,
             CASE WHEN octet_length(state::text) <= 8388608 THEN state::text END AS state_json,
             CASE WHEN octet_length(last_tick_result::text) <= 524288 THEN last_tick_result::text END AS last_tick_json,
             last_tick_result IS NOT NULL AS has_tick_result
         FROM game_worlds WHERE world_id = $1 FOR UPDATE",
    )
    .bind(world_id)
    .fetch_optional(connection)
    .await
    .map_err(ApiError::database)?
    .ok_or_else(|| conflict("The world is not initialized."))?;
    let state: WorldState = decode(row.state_json.as_deref(), MAX_WORLD_BYTES)?;
    validate_world(&state).map_err(|_| ApiError::internal("game_stored_world_validation"))?;
    if row.schema_version != state.schema_version as i32
        || row.content_revision != state.content_revision
        || row.revision != number(state.revision)?
        || row.tick != number(state.tick)?
    {
        return Err(ApiError::internal("game_stored_world_metadata"));
    }
    let last_tick: Option<TickReceipt> = if row.has_tick_result {
        Some(decode(row.last_tick_json.as_deref(), MAX_RESULT_BYTES)?)
    } else {
        None
    };
    if last_tick.as_ref().is_some_and(|receipt| {
        receipt.world_id != world_id
            || receipt.tick != state.tick
            || receipt.tick == 0
            || receipt.world_revision == 0
            || receipt.world_revision > state.revision
            || validate_events(&receipt.events).is_err()
            || validate_routing(
                &receipt.routed_events,
                &receipt.events,
                world_id,
                receipt.world_revision,
                &state,
            )
            .is_err()
    }) || (state.tick > 0 && last_tick.is_none())
    {
        return Err(ApiError::internal("game_stored_tick_result"));
    }
    Ok(WorldSnapshot {
        world_id: row.world_id,
        state,
        last_tick,
    })
}

#[derive(Clone, sqlx::FromRow)]
struct CharacterRow {
    account_id: Uuid,
    actor_id: String,
    world_id: Uuid,
    revision: i64,
    last_sequence: i64,
}

async fn lock_characters(
    connection: &mut PgConnection,
    world: &WorldSnapshot,
) -> Result<Vec<CharacterRow>, ApiError> {
    let rows = sqlx::query_as::<_, CharacterRow>(
        "SELECT account_id, actor_id, world_id, revision, last_sequence FROM game_characters
         WHERE world_id = $1 ORDER BY actor_id LIMIT 257 FOR UPDATE",
    )
    .bind(world.world_id)
    .fetch_all(connection)
    .await
    .map_err(ApiError::database)?;
    if rows.len() != world.state.characters.len() {
        return Err(ApiError::internal("game_stored_character_index"));
    }
    for row in &rows {
        character_snapshot(world, row)?;
    }
    Ok(rows)
}

fn character_snapshot(
    world: &WorldSnapshot,
    row: &CharacterRow,
) -> Result<CharacterSnapshot, ApiError> {
    let actor = ActorId::new(row.actor_id.clone())
        .map_err(|_| ApiError::internal("game_stored_actor_id"))?;
    let state = world
        .state
        .characters
        .get(&actor)
        .ok_or_else(|| ApiError::internal("game_stored_character_missing"))?;
    if row.world_id != world.world_id
        || row.revision <= 0
        || row.revision as u64 > world.state.revision
        || row.last_sequence < 0
        || row.last_sequence >= row.revision
        || row.last_sequence as u64 != state.last_command_sequence
    {
        return Err(ApiError::internal("game_stored_character_metadata"));
    }
    Ok(CharacterSnapshot {
        account_id: row.account_id,
        world_id: row.world_id,
        revision: row.revision as u64,
        last_sequence: row.last_sequence as u64,
        state: state.clone(),
    })
}

fn owned_character<'a>(
    characters: &'a [CharacterRow],
    account_id: Uuid,
    actor: &ActorId,
) -> Result<&'a CharacterRow, ApiError> {
    characters
        .iter()
        .find(|row| row.account_id == account_id && row.actor_id == actor.as_str())
        .ok_or_else(|| conflict("The character does not belong to this account and world."))
}

#[derive(sqlx::FromRow)]
struct AccountRow {
    account_id: Uuid,
    login_name: String,
}

async fn lock_account(
    connection: &mut PgConnection,
    authentication: AuthTokenDigest,
) -> Result<AccountRow, ApiError> {
    // This is only an identity lookup. Authentication is rechecked after all ownership locks.
    let account: Uuid =
        sqlx::query_scalar("SELECT account_id FROM account_sessions WHERE token_digest = $1")
            .bind(authentication.0.as_slice())
            .fetch_optional(&mut *connection)
            .await
            .map_err(ApiError::database)?
            .ok_or_else(ApiError::unauthenticated)?;
    sqlx::query_as("SELECT account_id, login_name FROM accounts WHERE account_id = $1 FOR UPDATE")
        .bind(account)
        .fetch_optional(connection)
        .await
        .map_err(ApiError::database)?
        .ok_or_else(ApiError::unauthenticated)
}

async fn lock_authentication(
    connection: &mut PgConnection,
    account_id: Uuid,
    authentication: AuthTokenDigest,
) -> Result<(), ApiError> {
    // Same-account login/pruning takes the account lock first. Shared token locks serialize with
    // logout DELETE; a successful logout cannot race past a transaction still using that token.
    let digests: Vec<Vec<u8>> = sqlx::query_scalar(
        "SELECT token_digest FROM account_sessions WHERE account_id = $1
         ORDER BY token_digest LIMIT 6 FOR SHARE",
    )
    .bind(account_id)
    .fetch_all(&mut *connection)
    .await
    .map_err(ApiError::database)?;
    if digests.len() > 5 || digests.iter().any(|digest| digest.len() != 32) {
        return Err(ApiError::internal("game_account_session_bounds"));
    }
    validate_authentication(connection, account_id, authentication).await
}

async fn validate_authentication(
    connection: &mut PgConnection,
    account_id: Uuid,
    authentication: AuthTokenDigest,
) -> Result<(), ApiError> {
    let valid: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM account_sessions WHERE account_id = $1 AND token_digest = $2
         AND expires_at > clock_timestamp())",
    )
    .bind(account_id)
    .bind(authentication.0.as_slice())
    .fetch_one(connection)
    .await
    .map_err(ApiError::database)?;
    if !valid {
        return Err(ApiError::unauthenticated());
    }
    Ok(())
}

#[derive(sqlx::FromRow)]
struct SessionRow {
    account_id: Uuid,
    actor_id: String,
    world_id: Uuid,
    session_id: Uuid,
    token_digest: Vec<u8>,
    heartbeat_at_unix_ms: i64,
    expires_at_unix_ms: i64,
}

impl SessionRow {
    fn matches(&self, access: &SessionAccess) -> Result<(), ApiError> {
        if self.session_id != access.session_id
            || self.world_id != access.world_id
            || self.actor_id != access.actor_id.as_str()
            || self.token_digest != access.authentication.0
        {
            return Err(conflict(
                "The game session does not belong to this authentication session.",
            ));
        }
        Ok(())
    }

    fn public(self) -> Result<GameSession, ApiError> {
        Ok(GameSession {
            account_id: self.account_id,
            actor_id: ActorId::new(self.actor_id)
                .map_err(|_| ApiError::internal("game_session_actor"))?,
            world_id: self.world_id,
            session_id: self.session_id,
            heartbeat_at_unix_ms: self.heartbeat_at_unix_ms,
            expires_at_unix_ms: self.expires_at_unix_ms,
        })
    }
}

async fn lock_session(
    connection: &mut PgConnection,
    account_id: Uuid,
) -> Result<Option<SessionRow>, ApiError> {
    sqlx::query_as(
        "SELECT account_id, actor_id, world_id, session_id, token_digest,
             floor(extract(epoch FROM heartbeat_at) * 1000)::bigint AS heartbeat_at_unix_ms,
             floor(extract(epoch FROM expires_at) * 1000)::bigint AS expires_at_unix_ms
         FROM game_sessions WHERE account_id = $1 FOR UPDATE",
    )
    .bind(account_id)
    .fetch_optional(connection)
    .await
    .map_err(ApiError::database)
}

async fn session_is_live(
    connection: &mut PgConnection,
    session_id: Uuid,
) -> Result<bool, ApiError> {
    sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM game_sessions WHERE session_id = $1
         AND expires_at > clock_timestamp())",
    )
    .bind(session_id)
    .fetch_one(connection)
    .await
    .map_err(ApiError::database)
}

async fn put_session(
    connection: &mut PgConnection,
    access: &SessionAccess,
    account_id: Uuid,
    milliseconds: i64,
) -> Result<SessionRow, ApiError> {
    let session = sqlx::query_as(
        "WITH stamp AS MATERIALIZED (SELECT clock_timestamp() AS now)
         INSERT INTO game_sessions
             (account_id, actor_id, world_id, session_id, token_digest, created_at, heartbeat_at, expires_at)
         SELECT $1, $2, $3, $4, $5, stamp.now, stamp.now,
                least(s.expires_at, stamp.now + $6::bigint * INTERVAL '1 millisecond')
         FROM stamp, account_sessions s
         WHERE s.account_id = $1 AND s.token_digest = $5 AND s.expires_at > stamp.now
         ON CONFLICT (account_id) DO UPDATE SET
             actor_id = EXCLUDED.actor_id, world_id = EXCLUDED.world_id,
             session_id = EXCLUDED.session_id, token_digest = EXCLUDED.token_digest,
             created_at = CASE WHEN game_sessions.session_id = EXCLUDED.session_id
                               THEN game_sessions.created_at ELSE EXCLUDED.created_at END,
             heartbeat_at = EXCLUDED.heartbeat_at, expires_at = EXCLUDED.expires_at
         WHERE game_sessions.session_id <> EXCLUDED.session_id
             OR game_sessions.expires_at > EXCLUDED.heartbeat_at
         RETURNING account_id, actor_id, world_id, session_id, token_digest,
             floor(extract(epoch FROM heartbeat_at) * 1000)::bigint AS heartbeat_at_unix_ms,
             floor(extract(epoch FROM expires_at) * 1000)::bigint AS expires_at_unix_ms",
    )
    .bind(account_id)
    .bind(access.actor_id.as_str())
    .bind(access.world_id)
    .bind(access.session_id)
    .bind(access.authentication.0.as_slice())
    .bind(milliseconds)
    .fetch_optional(&mut *connection)
    .await
    .map_err(ApiError::database)?;
    if let Some(session) = session {
        return Ok(session);
    }
    validate_authentication(connection, account_id, access.authentication).await?;
    Err(conflict("The game session lease expired during renewal."))
}

struct PlayerRows {
    account: AccountRow,
    characters: Vec<CharacterRow>,
    session: SessionRow,
}

async fn lock_player(
    connection: &mut PgConnection,
    world: &WorldSnapshot,
    access: &SessionAccess,
) -> Result<PlayerRows, ApiError> {
    let account = lock_account(connection, access.authentication).await?;
    let characters = lock_characters(connection, world).await?;
    lock_authentication(connection, account.account_id, access.authentication).await?;
    owned_character(&characters, account.account_id, &access.actor_id)?;
    let session = lock_session(connection, account.account_id)
        .await?
        .ok_or_else(|| conflict("An active game session is required."))?;
    session.matches(access)?;
    validate_player(connection, access, account.account_id).await?;
    Ok(PlayerRows {
        account,
        characters,
        session,
    })
}

async fn validate_player(
    connection: &mut PgConnection,
    access: &SessionAccess,
    account_id: Uuid,
) -> Result<(), ApiError> {
    validate_authentication(connection, account_id, access.authentication).await?;
    if !session_is_live(connection, access.session_id).await? {
        return Err(conflict("The game session lease has expired."));
    }
    Ok(())
}

fn validate_access(access: &SessionAccess) -> Result<(), ApiError> {
    codec::uuid(access.world_id)?;
    codec::uuid(access.session_id)
}

fn lease_milliseconds(duration: Duration, maximum: Duration) -> Result<i64, ApiError> {
    if duration < Duration::from_millis(1) || duration > maximum {
        return Err(ApiError::invalid(
            "The infrastructure lease duration is out of range.",
        ));
    }
    i64::try_from(duration.as_millis())
        .map_err(|_| ApiError::invalid("The infrastructure lease duration is out of range."))
}

fn stale_fence() -> ApiError {
    conflict("The world ownership lease is expired, replaced, or belongs to another repository.")
}

async fn ensure_fence(connection: &mut PgConnection, lease: &WorldLease) -> Result<(), ApiError> {
    let valid: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM game_worlds
         WHERE world_id = $1 AND lease_owner = $2 AND lease_fence = $3
             AND lease_expires_at > clock_timestamp())",
    )
    .bind(lease.world_id)
    .bind(lease.owner_id)
    .bind(number(lease.fence)?)
    .fetch_one(connection)
    .await
    .map_err(ApiError::database)?;
    if !valid {
        return Err(stale_fence());
    }
    Ok(())
}

fn validate_transition(before: &WorldState, after: &WorldState, tick: u64) -> Result<(), ApiError> {
    if before.schema_version != after.schema_version
        || before.content_revision != after.content_revision
        || before.runtime.ui_version != after.runtime.ui_version
        || before.revision != after.revision
        || after.tick != tick
        || !before.characters.keys().eq(after.characters.keys())
        || before.characters.iter().any(|(id, old)| {
            after.characters[id].last_command_sequence != old.last_command_sequence
        })
    {
        return Err(ApiError::internal("game_callback_metadata"));
    }
    for (id, old) in &before.characters {
        old.runtime
            .validate_ledger_successor(&after.characters[id].runtime)
            .map_err(|_| ApiError::internal("game_callback_reward_ledger"))?;
    }
    validate_world(after).map_err(|_| ApiError::internal("game_callback_state"))
}

fn changed_characters(
    before: &WorldState,
    after: &WorldState,
    characters: &[CharacterRow],
) -> Result<Vec<CharacterRow>, ApiError> {
    let mut changed = Vec::new();
    for row in characters {
        let actor = ActorId::new(row.actor_id.clone())
            .map_err(|_| ApiError::internal("game_stored_actor_id"))?;
        if before.characters[&actor] != after.characters[&actor] {
            let mut row = row.clone();
            row.revision = number(increment(row.revision as u64)?)?;
            row.last_sequence = number(after.characters[&actor].last_command_sequence)?;
            changed.push(row);
        }
    }
    Ok(changed)
}

async fn save_world(
    connection: &mut PgConnection,
    lease: &WorldLease,
    state: &WorldState,
    previous_revision: u64,
    json: String,
    tick_result: Option<String>,
) -> Result<(), ApiError> {
    let changed = sqlx::query(
        "UPDATE game_worlds SET state = $2::jsonb, revision = $3, tick = $4,
             last_tick_result = coalesce($5::jsonb, last_tick_result)
         WHERE world_id = $1 AND lease_owner = $6 AND lease_fence = $7
             AND lease_expires_at > clock_timestamp() AND revision = $8",
    )
    .bind(lease.world_id)
    .bind(json)
    .bind(number(state.revision)?)
    .bind(number(state.tick)?)
    .bind(tick_result)
    .bind(lease.owner_id)
    .bind(number(lease.fence)?)
    .bind(number(previous_revision)?)
    .execute(connection)
    .await
    .map_err(ApiError::database)?;
    if changed.rows_affected() != 1 {
        return Err(stale_fence());
    }
    Ok(())
}

async fn save_characters(
    connection: &mut PgConnection,
    world_id: Uuid,
    changed: &[CharacterRow],
) -> Result<(), ApiError> {
    if changed.is_empty() {
        return Ok(());
    }
    let actors: Vec<&str> = changed.iter().map(|row| row.actor_id.as_str()).collect();
    let revisions: Vec<i64> = changed.iter().map(|row| row.revision).collect();
    let sequences: Vec<i64> = changed.iter().map(|row| row.last_sequence).collect();
    let result = sqlx::query(
        "UPDATE game_characters c SET revision = changed.revision, last_sequence = changed.sequence
         FROM unnest($1::text[], $2::bigint[], $3::bigint[]) AS changed(actor_id, revision, sequence)
         WHERE c.world_id = $4 AND c.actor_id = changed.actor_id",
    )
    .bind(actors)
    .bind(revisions)
    .bind(sequences)
    .bind(world_id)
    .execute(connection)
    .await
    .map_err(ApiError::database)?;
    if result.rows_affected() != changed.len() as u64 {
        return Err(ApiError::internal("game_character_update_count"));
    }
    Ok(())
}

#[derive(sqlx::FromRow)]
struct JournalRow {
    actor_id: String,
    world_id: Uuid,
    sequence: i64,
    intent_version: i16,
    intent_hash: Vec<u8>,
    world_revision: i64,
    character_revision: i64,
    result_json: Option<String>,
}

async fn journal_result(
    connection: &mut PgConnection,
    world: &WorldSnapshot,
    character: &CharacterRow,
    command: &GameCommand,
    hash: [u8; 32],
) -> Result<Option<CommandReceipt>, ApiError> {
    let Some(row) = sqlx::query_as::<_, JournalRow>(
        "SELECT actor_id, world_id, sequence, intent_version, intent_hash, world_revision,
             character_revision,
             CASE WHEN octet_length(committed_result::text) <= 524288
                  THEN committed_result::text END AS result_json
         FROM processed_game_commands WHERE account_id = $1 AND operation_id = $2 FOR UPDATE",
    )
    .bind(character.account_id)
    .bind(command.operation_id)
    .fetch_optional(connection)
    .await
    .map_err(ApiError::database)?
    else {
        return Ok(None);
    };
    if row.intent_version != 1 {
        return Err(ApiError::internal("game_journal_intent_version"));
    }
    if row.actor_id != character.actor_id
        || row.world_id != world.world_id
        || row.sequence != number(command.sequence)?
        || row.intent_hash != hash
    {
        return Err(conflict(
            "The operation ID was already committed with a different intent or sequence.",
        ));
    }
    let receipt: CommandReceipt = decode(row.result_json.as_deref(), MAX_RESULT_BYTES)?;
    if receipt.operation_id != command.operation_id
        || receipt.sequence != command.sequence
        || receipt.world_id != world.world_id
        || receipt.character.actor_id.as_str() != character.actor_id
        || receipt.character.last_command_sequence != receipt.sequence
        || receipt.sequence > character.last_sequence as u64
        || receipt.world_revision != row.world_revision as u64
        || row.world_revision <= 0
        || receipt.world_revision > world.state.revision
        || receipt.world_tick > world.state.tick
        || receipt.character_revision != row.character_revision as u64
        || row.character_revision <= 0
        || receipt.character_revision > character.revision as u64
        || validate_character(&receipt.character, receipt.world_tick).is_err()
        || validate_events(&receipt.events).is_err()
        || validate_routing(
            &receipt.routed_events,
            &receipt.events,
            world.world_id,
            receipt.world_revision,
            &world.state,
        )
        .is_err()
    {
        return Err(ApiError::internal("game_journal_result_metadata"));
    }
    Ok(Some(receipt))
}

enum EventOutput {
    Legacy(Vec<GameEvent>),
    Routed(Vec<ActorEvent>),
}

enum TickAuthority {
    Unrestricted,
    Strict(Vec<SessionAccess>),
    Live(Vec<SessionAccess>),
}

fn event_id(world: Uuid, revision: u64, index: usize, actor: &ActorId) -> String {
    let mut hash = Sha256::new();
    hash.update(b"clubscape.actor-event.v1\0");
    hash.update(world.as_bytes());
    hash.update(revision.to_be_bytes());
    hash.update((index as u64).to_be_bytes());
    hash.update(actor.as_str().as_bytes());
    format!("event:{:x}", hash.finalize())
}

fn commit_events(
    output: EventOutput,
    world: &WorldSnapshot,
) -> Result<(Vec<GameEvent>, Vec<CommittedActorEvent>), ApiError> {
    match &output {
        EventOutput::Legacy(events) => validate_events(events)?,
        EventOutput::Routed(events) if events.len() > 1024 => {
            return Err(ApiError::invalid(
                "The event result exceeds its storage bounds.",
            ));
        }
        EventOutput::Routed(_) => {}
    }
    let (legacy, routed) = match output {
        EventOutput::Legacy(events) => (events, Vec::new()),
        EventOutput::Routed(events) => (
            Vec::new(),
            events
                .into_iter()
                .enumerate()
                .map(|(index, event)| CommittedActorEvent {
                    event_id: event_id(
                        world.world_id,
                        world.state.revision,
                        index,
                        &event.actor_id,
                    ),
                    actor_id: event.actor_id,
                    event: event.event,
                })
                .collect(),
        ),
    };
    validate_routing(
        &routed,
        &legacy,
        world.world_id,
        world.state.revision,
        &world.state,
    )?;
    Ok((legacy, routed))
}

fn validate_routing(
    routed: &[CommittedActorEvent],
    legacy: &[GameEvent],
    world: Uuid,
    revision: u64,
    state: &WorldState,
) -> Result<(), ApiError> {
    if routed.len() + legacy.len() > 1024
        || (!routed.is_empty() && !legacy.is_empty())
        || routed.iter().enumerate().any(|(index, event)| {
            !state.characters.contains_key(&event.actor_id)
                || event.event_id != event_id(world, revision, index, &event.actor_id)
        })
    {
        return Err(ApiError::internal("game_event_routing_integrity"));
    }
    Ok(())
}

async fn lock_tick_sessions(
    connection: &mut PgConnection,
    world: &WorldSnapshot,
    sessions: &[SessionAccess],
) -> Result<(), ApiError> {
    // World locking already excludes character/session writers. Token-only shared locks do not
    // acquire account locks afterward and therefore cannot invert account login's lock order.
    let _: Vec<Vec<u8>> = sqlx::query_scalar(
        "SELECT a.token_digest FROM account_sessions a JOIN game_sessions s
             ON s.account_id = a.account_id AND s.token_digest = a.token_digest
         WHERE s.world_id = $1 ORDER BY a.token_digest FOR SHARE OF a",
    )
    .bind(world.world_id)
    .fetch_all(&mut *connection)
    .await
    .map_err(ApiError::database)?;
    check_tick_sessions(connection, world, sessions).await
}

async fn check_tick_sessions(
    connection: &mut PgConnection,
    world: &WorldSnapshot,
    sessions: &[SessionAccess],
) -> Result<(), ApiError> {
    let actors: std::collections::BTreeSet<_> =
        sessions.iter().map(|session| &session.actor_id).collect();
    if sessions.len() != world.state.characters.len()
        || actors.len() != sessions.len()
        || !actors.into_iter().eq(world.state.characters.keys())
        || sessions
            .iter()
            .any(|session| session.world_id != world.world_id)
    {
        return Err(ApiError::new(
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            clubscape_protocol::ErrorCode::Unavailable,
            "The supplied strict tick session set is incomplete.",
        ));
    }
    for access in sessions {
        let valid: bool = sqlx::query_scalar(
            "SELECT EXISTS (SELECT 1 FROM game_sessions s JOIN account_sessions a
                ON a.account_id = s.account_id AND a.token_digest = s.token_digest
             WHERE s.world_id = $1 AND s.actor_id = $2 AND s.session_id = $3 AND s.token_digest = $4
                AND s.expires_at > clock_timestamp() AND a.expires_at > clock_timestamp())",
        )
        .bind(world.world_id)
        .bind(access.actor_id.as_str())
        .bind(access.session_id)
        .bind(access.authentication.0.as_slice())
        .fetch_one(&mut *connection)
        .await
        .map_err(ApiError::database)?;
        if !valid {
            return Err(ApiError::new(
                axum::http::StatusCode::SERVICE_UNAVAILABLE,
                clubscape_protocol::ErrorCode::Unavailable,
                "A supplied strict tick session is no longer live.",
            ));
        }
    }
    Ok(())
}

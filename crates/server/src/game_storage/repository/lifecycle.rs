use super::*;
use crate::game_storage::{
    LifecycleCommit, LifecycleReceipt, LiveSessionAction, SessionLoss, VerifiedConnections,
    WorldControlCommit,
};
use clubscape_world_engine::LifecycleTransition;

impl GameStore {
    /// Operator-authorized UI metadata migration, conditional on the exact old artifact pin.
    pub async fn migrate_ui_content<F>(
        &self,
        lease: &WorldLease,
        from_hash: String,
        to_hash: String,
        revision: String,
        migrate: F,
    ) -> Result<WorldSnapshot, GameStorageError>
    where
        F: FnOnce(&mut WorldState) -> GameResult<()> + Send + 'static,
    {
        self.local_lease(lease)?;
        if !codec::text(&revision, 256)
            || from_hash == to_hash
            || [&from_hash, &to_hash].iter().any(|value| {
                value.len() != 64
                    || !value
                        .bytes()
                        .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
            })
        {
            return Err(ApiError::invalid(
                "Exact source and destination artifact hashes are required.",
            )
            .into());
        }
        let lease = lease.clone();
        database::run(&self.pool, "game_ui_migration", true, move |connection| Box::pin(async move {
            let mut transaction = connection.begin().await.map_err(ApiError::database)?;
            let mut world = lock_world(&mut transaction, lease.world_id).await?;
            ensure_fence(&mut transaction, &lease).await?;
            lock_characters(&mut transaction, &world).await?;
            let current: Option<String> = sqlx::query_scalar("SELECT runtime_artifact_sha256 FROM game_worlds WHERE world_id = $1")
                .bind(lease.world_id).fetch_one(&mut *transaction).await.map_err(ApiError::database)?;
            if current.as_deref() == Some(&to_hash) && world.state.content_revision == revision {
                let audit: Option<(String, i64)> = sqlx::query_as(
                    "SELECT from_artifact, world_revision FROM game_content_migrations WHERE world_id = $1 AND to_artifact = $2",
                ).bind(lease.world_id).bind(&to_hash).fetch_optional(&mut *transaction).await.map_err(ApiError::database)?;
                if audit.is_none_or(|(from, at)| from != from_hash || at <= 0 || at as u64 > world.state.revision) {
                    return Err(conflict("This target does not have the explicitly requested prior migration receipt."));
                }
                if world.state.runtime.ui_version != Some(clubscape_game_types::UI_STATE_VERSION)
                    || world.state.characters.values().any(|character| character.runtime.ui.is_none()) {
                    return Err(ApiError::internal("game_ui_migration_incomplete"));
                }
                ensure_fence(&mut transaction, &lease).await?;
                transaction.commit().await.map_err(ApiError::database)?;
                return Ok(world);
            }
            if current.as_deref() != Some(&from_hash) { return Err(conflict("The world does not have the explicitly authorized old artifact.")); }
            let previous = world.state.clone();
            world.state.content_revision = revision.clone();
            migrate(&mut world.state).map_err(callback_error)?;
            let mut preserved = world.state.clone();
            preserved.content_revision = previous.content_revision.clone();
            preserved.runtime.ui_version = previous.runtime.ui_version;
            preserved.runtime.audio_authority_version = previous.runtime.audio_authority_version;
            for (id, character) in &mut preserved.characters {
                let old = previous.characters.get(id).ok_or_else(|| ApiError::internal("game_ui_migration_actor"))?;
                if old.runtime.audio_authority.is_some() && character.runtime.audio_authority != old.runtime.audio_authority {
                    return Err(ApiError::internal("game_ui_migration_changed_audio_history"));
                }
                if old.runtime.audio_authority.is_none() && character.runtime.audio_authority.as_ref().is_some_and(|audio| {
                    audio.history != clubscape_game_types::MusicHistoryStatus::LegacyUntracked
                        || audio.tracked_from_tick != previous.tick
                }) {
                    return Err(ApiError::internal("game_ui_migration_fabricated_audio_history"));
                }
                character.runtime.audio_authority = old.runtime.audio_authority.clone();
                if old.runtime.ui.is_some() && character.runtime.ui != old.runtime.ui {
                    return Err(ApiError::internal("game_ui_migration_changed_history"));
                }
                character.runtime.ui = old.runtime.ui.clone();
            }
            if preserved != previous { return Err(ApiError::internal("game_ui_migration_changed_gameplay")); }
            validate_world(&world.state)?;
            world.state.revision = increment(previous.revision)?;
            let json = encode(&world.state, MAX_WORLD_BYTES)?;
            let count = sqlx::query(
                "UPDATE game_worlds SET state = $2::jsonb, content_revision = $3, revision = $4,
                     runtime_artifact_sha256 = $5 WHERE world_id = $1 AND runtime_artifact_sha256 = $6
                     AND lease_owner = $7 AND lease_fence = $8 AND lease_expires_at > clock_timestamp()",
            ).bind(lease.world_id).bind(json).bind(revision).bind(number(world.state.revision)?)
                .bind(&to_hash).bind(&from_hash).bind(lease.owner_id).bind(number(lease.fence)?)
                .execute(&mut *transaction).await.map_err(ApiError::database)?.rows_affected();
            if count != 1 { return Err(stale_fence()); }
            sqlx::query("UPDATE game_characters SET revision = revision + 1 WHERE world_id = $1")
                .bind(lease.world_id).execute(&mut *transaction).await.map_err(ApiError::database)?;
            sqlx::query("INSERT INTO game_content_migrations (world_id, from_artifact, to_artifact, world_revision) VALUES ($1,$2,$3,$4)")
                .bind(lease.world_id).bind(from_hash).bind(to_hash).bind(number(world.state.revision)?)
                .execute(&mut *transaction).await.map_err(ApiError::database)?;
            ensure_fence(&mut transaction, &lease).await?;
            transaction.commit().await.map_err(ApiError::database)?;
            Ok(world)
        })).await.map_err(Into::into)
    }
    /// Lifecycle and its auth/session effects commit together, without a gameplay sequence.
    pub async fn apply_session_lifecycle<F>(
        &self,
        lease: &WorldLease,
        authentication: AuthTokenDigest,
        operation_id: Uuid,
        action: LiveSessionAction,
        duration: Duration,
        apply: F,
    ) -> Result<LifecycleCommit, GameStorageError>
    where
        F: Fn(&mut WorldState, &ActorId, LifecycleTransition) -> GameResult<Vec<ActorEvent>>
            + Send
            + 'static,
    {
        self.local_lease(lease)?;
        codec::uuid(operation_id)?;
        if let LiveSessionAction::Leave { session_id } = &action {
            codec::uuid(*session_id)?;
        }
        let milliseconds = lease_milliseconds(duration, MAX_SESSION_LEASE)?;
        let hash: [u8; 32] = Sha256::digest(encode(&action, 1024)?.as_bytes()).into();
        let lease = lease.clone();
        database::run(&self.pool, "game_session_lifecycle", true, move |connection| Box::pin(async move {
            let mut transaction = connection.begin().await.map_err(ApiError::database)?;
            let mut world = lock_world(&mut transaction, lease.world_id).await?;
            ensure_fence(&mut transaction, &lease).await?;
            let account = lock_account(&mut transaction, authentication).await?;
            let mut rows = lock_characters(&mut transaction, &world).await?;
            lock_authentication(&mut transaction, account.account_id, authentication).await?;
            let existing = lock_session(&mut transaction, account.account_id).await?;
            let saved: Option<(Vec<u8>, Vec<u8>, String)> = sqlx::query_as(
                "SELECT intent_hash, auth_digest, committed_result::text FROM game_lifecycle_commands
                 WHERE account_id = $1 AND operation_id = $2 FOR UPDATE",
            ).bind(account.account_id).bind(operation_id).fetch_optional(&mut *transaction)
                .await.map_err(ApiError::database)?;
            if let Some((stored_hash, digest, json)) = saved {
                if stored_hash != hash || digest != authentication.0 {
                    return Err(conflict("The lifecycle operation ID has a different intent or authentication owner."));
                }
                let receipt: LifecycleReceipt = decode(Some(&json), MAX_RESULT_BYTES)?;
                if receipt.operation_id != operation_id || receipt.account_id != account.account_id
                    || receipt.world_id != lease.world_id || receipt.action != action
                    || receipt.world_revision > world.state.revision
                {
                    return Err(ApiError::internal("game_lifecycle_receipt"));
                }
                let expected_actor = rows.iter().find(|row| row.account_id == account.account_id)
                    .map(|row| ActorId::new(row.actor_id.clone()).map_err(|_| ApiError::internal("game_actor")))
                    .transpose()?;
                if receipt.actor_id != expected_actor
                    || receipt.session.as_ref().is_some_and(|session| {
                        session.account_id != account.account_id || session.world_id != lease.world_id
                            || Some(&session.actor_id) != expected_actor.as_ref()
                            || session.session_id.is_nil()
                            || session.expires_at_unix_ms < session.heartbeat_at_unix_ms
                    })
                    || (!matches!(action, LiveSessionAction::Join) && receipt.session.is_some())
                {
                    return Err(ApiError::internal("game_lifecycle_receipt_ownership"));
                }
                validate_routing(&receipt.events, &[], world.world_id, receipt.world_revision, &world.state)?;
                if let LiveSessionAction::Join = action {
                    let returned = receipt.session.as_ref().ok_or_else(|| ApiError::internal("game_lifecycle_session"))?;
                    let current = existing.as_ref().ok_or_else(|| conflict("Rejoin using a new lifecycle operation ID."))?;
                    current.matches(&returned.access(authentication))?;
                    if current.session_id != returned.session_id || !session_is_live(&mut transaction, current.session_id).await? {
                        return Err(conflict("Rejoin using a new lifecycle operation ID."));
                    }
                }
                let character = rows.iter().find(|row| row.account_id == account.account_id)
                    .map(|row| character_snapshot(&world, row)).transpose()?;
                validate_authentication(&mut transaction, account.account_id, authentication).await?;
                ensure_fence(&mut transaction, &lease).await?;
                transaction.commit().await.map_err(ApiError::database)?;
                return Ok(LifecycleCommit { receipt, snapshot: world, character, duplicate: true });
            }
            let used: bool = sqlx::query_scalar(
                "SELECT EXISTS (SELECT 1 FROM processed_game_commands WHERE account_id = $1 AND operation_id = $2)",
            ).bind(account.account_id).bind(operation_id).fetch_one(&mut *transaction).await.map_err(ApiError::database)?;
            if used { return Err(conflict("The operation ID belongs to a gameplay command.")); }
            let actor = rows.iter().find(|row| row.account_id == account.account_id)
                .map(|row| ActorId::new(row.actor_id.clone()).map_err(|_| ApiError::internal("game_actor")))
                .transpose()?;
            let before = world.state.clone();
            let mut emitted = Vec::new();
            let mut session = None;
            match &action {
                LiveSessionAction::Join => {
                    let actor = actor.as_ref().ok_or_else(|| conflict("Create a character before joining."))?;
                    let mut id = Uuid::new_v4();
                    if let Some(previous) = &existing {
                        let token_live: bool = sqlx::query_scalar(
                            "SELECT EXISTS (SELECT 1 FROM account_sessions WHERE account_id = $1
                             AND token_digest = $2 AND expires_at > clock_timestamp())",
                        ).bind(account.account_id).bind(&previous.token_digest)
                            .fetch_one(&mut *transaction).await.map_err(ApiError::database)?;
                        let lease_live = session_is_live(&mut transaction, previous.session_id).await?;
                        if token_live && lease_live {
                            if previous.token_digest != authentication.0 {
                                return Err(conflict("Another authentication session owns the active game lease."));
                            }
                            id = previous.session_id;
                        } else {
                            let transition = if token_live { LifecycleTransition::TransportLost }
                                else { LifecycleTransition::AuthenticationRevoked };
                            emitted.extend(apply(&mut world.state, actor, transition).map_err(callback_error)?);
                        }
                    }
                    let transition = if existing.is_some() { LifecycleTransition::Rejoin } else { LifecycleTransition::Join };
                    emitted.extend(apply(&mut world.state, actor, transition).map_err(callback_error)?);
                    session = Some(put_session(&mut transaction, &SessionAccess {
                        world_id: lease.world_id, actor_id: actor.clone(), session_id: id, authentication,
                    }, account.account_id, milliseconds).await?.public()?);
                }
                LiveSessionAction::Leave { session_id } => {
                    let actor = actor.as_ref().ok_or_else(|| conflict("There is no owned character to leave."))?;
                    let current = existing.as_ref().ok_or_else(|| conflict("There is no matching game session."))?;
                    current.matches(&SessionAccess { world_id: lease.world_id, actor_id: actor.clone(), session_id: *session_id, authentication })?;
                    emitted.extend(apply(&mut world.state, actor, LifecycleTransition::RequestedLogout).map_err(callback_error)?);
                    sqlx::query("DELETE FROM game_sessions WHERE account_id = $1 AND session_id = $2")
                        .bind(account.account_id).bind(session_id).execute(&mut *transaction).await.map_err(ApiError::database)?;
                }
                LiveSessionAction::Logout => {
                    let owns = existing.as_ref().is_none_or(|session| session.token_digest == authentication.0);
                    if owns && let Some(actor) = &actor {
                        emitted.extend(apply(&mut world.state, actor, LifecycleTransition::RequestedLogout).map_err(callback_error)?);
                        sqlx::query("DELETE FROM game_sessions WHERE account_id = $1 AND token_digest = $2")
                            .bind(account.account_id).bind(authentication.0.as_slice())
                            .execute(&mut *transaction).await.map_err(ApiError::database)?;
                    }
                }
            }
            let events = save_control(&mut transaction, &lease, &before, &mut world, &mut rows, emitted).await?;
            let receipt = LifecycleReceipt {
                operation_id, account_id: account.account_id, world_id: lease.world_id,
                action: action.clone(), actor_id: actor,
                session, world_revision: world.state.revision, events,
            };
            let json = encode(&receipt, MAX_RESULT_BYTES)?;
            sqlx::query(
                "INSERT INTO game_lifecycle_commands (account_id, operation_id, world_id, auth_digest, intent_hash, committed_result)
                 VALUES ($1, $2, $3, $4, $5, $6::jsonb)",
            ).bind(account.account_id).bind(operation_id).bind(lease.world_id)
                .bind(authentication.0.as_slice()).bind(hash.as_slice()).bind(json)
                .execute(&mut *transaction).await.map_err(ApiError::database)?;
            validate_authentication(&mut transaction, account.account_id, authentication).await?;
            if matches!(action, LiveSessionAction::Logout) {
                crate::store::delete_session(&mut transaction, &authentication.0).await?;
            }
            let character = rows.iter().find(|row| row.account_id == account.account_id)
                .map(|row| character_snapshot(&world, row)).transpose()?;
            ensure_fence(&mut transaction, &lease).await?;
            transaction.commit().await.map_err(ApiError::database)?;
            Ok(LifecycleCommit { receipt, snapshot: world, character, duplicate: false })
        })).await.map_err(Into::into)
    }

    /// Trusted coordinator reconciliation; idempotent engine transitions must not grant progress.
    pub async fn control_world<F>(
        &self,
        lease: &WorldLease,
        apply: F,
    ) -> Result<WorldControlCommit, GameStorageError>
    where
        F: FnOnce(&mut WorldState) -> GameResult<Vec<ActorEvent>> + Send + 'static,
    {
        self.local_lease(lease)?;
        let lease = lease.clone();
        database::run(
            &self.pool,
            "game_world_lifecycle",
            true,
            move |connection| {
                Box::pin(async move {
                    let mut transaction = connection.begin().await.map_err(ApiError::database)?;
                    let mut world = lock_world(&mut transaction, lease.world_id).await?;
                    ensure_fence(&mut transaction, &lease).await?;
                    let mut rows = lock_characters(&mut transaction, &world).await?;
                    let before = world.state.clone();
                    let events = apply(&mut world.state).map_err(callback_error)?;
                    let events = save_control(
                        &mut transaction,
                        &lease,
                        &before,
                        &mut world,
                        &mut rows,
                        events,
                    )
                    .await?;
                    ensure_fence(&mut transaction, &lease).await?;
                    transaction.commit().await.map_err(ApiError::database)?;
                    Ok(WorldControlCommit {
                        snapshot: world,
                        events,
                    })
                })
            },
        )
        .await
        .map_err(Into::into)
    }
}

async fn save_control(
    connection: &mut PgConnection,
    lease: &WorldLease,
    before: &WorldState,
    world: &mut WorldSnapshot,
    rows: &mut [CharacterRow],
    events: Vec<ActorEvent>,
) -> Result<Vec<CommittedActorEvent>, ApiError> {
    validate_transition(before, &world.state, before.tick)?;
    if before == &world.state && events.is_empty() {
        return Ok(Vec::new());
    }
    world.state.revision = increment(before.revision)?;
    let (_, events) = commit_events(EventOutput::Routed(events), world)?;
    let changed = changed_characters(before, &world.state, rows)?;
    let json = encode(&world.state, MAX_WORLD_BYTES)?;
    save_world(connection, lease, &world.state, before.revision, json, None).await?;
    save_characters(connection, world.world_id, &changed).await?;
    for row in rows {
        if let Some(new) = changed
            .iter()
            .find(|changed| changed.actor_id == row.actor_id)
        {
            *row = new.clone();
        }
    }
    Ok(events)
}

pub(super) async fn connection_facts(
    connection: &mut PgConnection,
    world: &WorldSnapshot,
    sessions: &[SessionAccess],
    lock: bool,
) -> Result<VerifiedConnections, ApiError> {
    if sessions.len() > MAX_CHARACTERS {
        return Err(ApiError::invalid("Too many coordinator connections."));
    }
    if lock {
        let _: Vec<Vec<u8>> = sqlx::query_scalar(
            "SELECT a.token_digest FROM account_sessions a JOIN game_sessions s
                 ON a.account_id = s.account_id AND a.token_digest = s.token_digest
             WHERE s.world_id = $1 ORDER BY a.token_digest FOR SHARE OF a",
        )
        .bind(world.world_id)
        .fetch_all(&mut *connection)
        .await
        .map_err(ApiError::database)?;
    }
    let mut facts = VerifiedConnections::default();
    let mut seen = std::collections::BTreeSet::new();
    for access in sessions {
        if access.world_id != world.world_id
            || !world.state.characters.contains_key(&access.actor_id)
            || !seen.insert(&access.actor_id)
        {
            return Err(ApiError::internal("game_connection_identity"));
        }
        let (auth, lease): (bool, bool) = sqlx::query_as(
            "SELECT EXISTS (SELECT 1 FROM account_sessions a JOIN game_characters c ON c.account_id = a.account_id
                 WHERE a.token_digest = $1 AND c.actor_id = $2 AND c.world_id = $3 AND a.expires_at > clock_timestamp()),
             EXISTS (SELECT 1 FROM game_sessions WHERE world_id = $3 AND actor_id = $2
                 AND token_digest = $1 AND session_id = $4 AND expires_at > clock_timestamp())",
        ).bind(access.authentication.0.as_slice()).bind(access.actor_id.as_str()).bind(world.world_id)
            .bind(access.session_id).fetch_one(&mut *connection).await.map_err(ApiError::database)?;
        if auth && lease {
            facts.connected.insert(access.actor_id.clone());
        } else {
            facts.lost.insert(
                access.actor_id.clone(),
                if auth {
                    SessionLoss::TransportLost
                } else {
                    SessionLoss::AuthenticationRevoked
                },
            );
        }
    }
    Ok(facts)
}

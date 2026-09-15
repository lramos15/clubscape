mod config;
mod crypto;
mod database;
mod error;
mod game_service;
pub mod game_storage;
mod rate_limit;
mod store;
mod transport;
mod web_assets;

use std::{
    error::Error as _,
    future::Future,
    net::{IpAddr, SocketAddr},
    sync::Arc,
    time::{Duration, Instant},
};

use axum::{
    Json, Router,
    body::{Body, Bytes, to_bytes},
    extract::{ConnectInfo, Request, State},
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use clubscape_protocol::{
    AccountSnapshot, CAPABILITIES, ClientMessage, ErrorCode, GAMEPLAY_UNAVAILABLE_REASON,
    LoggedOut, MAX_REQUEST_BYTES, MEDIA_TYPE, Registered, ServerHello, client_message,
    normalize_login_name, server_message, validate_client_message,
};
use prost::Message;
use sqlx::{PgPool, migrate::Migrator, postgres::PgPoolOptions};
use tokio::{net::TcpListener, time::timeout};
use uuid::Uuid;

use crate::{
    crypto::{Passwords, token_digest},
    error::{ApiError, protobuf_response},
    rate_limit::RateLimiter,
};

pub use config::{Config, ConfigError, DEFAULT_BIND, UNVERSIONED_BUILD};
pub use error::{ServeError, StartupError};

static MIGRATIONS: Migrator = sqlx::migrate!("./migrations");
const BODY_TIMEOUT: Duration = Duration::from_secs(10);
const COMMAND_TIMEOUT: Duration = Duration::from_secs(10);

struct AppState {
    pool: PgPool,
    passwords: Passwords,
    limiter: RateLimiter,
    build_revision: String,
    web_assets: Option<Arc<web_assets::WebAssets>>,
    game: Option<game_service::GameHandle>,
}

/// A migrated, loopback-bound service. `serve` owns graceful listener/pool shutdown.
pub struct Service {
    listener: TcpListener,
    local_addr: SocketAddr,
    pool: PgPool,
    router: Router,
    game: Option<game_service::PreparedGame>,
}

impl Service {
    pub async fn bind(config: Config) -> Result<Self, StartupError> {
        let web_assets = config.web_root.as_deref().map(web_assets::WebAssets::load)
            .transpose().map_err(|error| {
                tracing::error!(event = "web_bundle_failure", error_kind = %error, "configured web bundle failed validation");
                StartupError::new("web_assets", "invalid_web_bundle")
            })?.map(Arc::new);
        let game_content = game_service::PreparedGame::load(&config)?;
        if let (Some(game), Some(web)) = (&game_content, &web_assets)
            && game.assets.overlaps(web)
        {
            return Err(StartupError::new(
                "game_content",
                "conflicting_static_routes",
            ));
        }
        let pool = database_pool(&config);
        let initialized = async {
            let acquired = timeout(database::IO_TIMEOUT, pool.acquire())
                .await
                .map_err(|_| StartupError::new("database", "connection_deadline"))?
                .map_err(|error| StartupError::database("database", &error))?;
            let mut connection = database::Connection::new(acquired);
            timeout(
                database::MIGRATION_TIMEOUT,
                MIGRATIONS.run_direct(&mut *connection),
            )
            .await
            .map_err(|_| StartupError::new("migrations", "database_deadline"))?
            .map_err(|error| match &error {
                sqlx::migrate::MigrateError::Execute(source)
                | sqlx::migrate::MigrateError::ExecuteMigration(source, _) => {
                    StartupError::database("migrations", source)
                }
                _ => StartupError::new("migrations", "migration_validation"),
            })?;
            if !connection.release().await {
                return Err(StartupError::new(
                    "migrations",
                    "connection_release_deadline",
                ));
            }
            let passwords = timeout(database::IO_TIMEOUT, Passwords::new())
                .await
                .map_err(|_| StartupError::new("passwords", "password_deadline"))?
                .map_err(|_| StartupError::new("passwords", "password_initialization"))?;
            let listener = TcpListener::bind(config.bind)
                .await
                .map_err(|_| StartupError::new("listener", "bind"))?;
            let local_addr = listener
                .local_addr()
                .map_err(|_| StartupError::new("listener", "local_address"))?;
            let game = match game_content {
                Some(content) => {
                    Some(game_service::PreparedGame::initialize(content, pool.clone()).await?)
                }
                None => None,
            };
            Ok((passwords, listener, local_addr, game))
        }
        .await;
        let (passwords, listener, local_addr, game) = match initialized {
            Ok(initialized) => initialized,
            Err(error) => {
                if !database::close_pool(pool).await {
                    StartupError::new("cleanup", "pool_close_deadline");
                }
                return Err(error);
            }
        };
        let state = Arc::new(AppState {
            pool: pool.clone(),
            passwords,
            limiter: RateLimiter::default(),
            build_revision: config.build_revision,
            web_assets,
            game: game.as_ref().map(|game| game.handle.clone()),
        });
        let router = Router::new()
            .route("/healthz", get(health))
            .route("/v1/rpc", post(rpc).fallback(method_not_allowed))
            .fallback(web_or_not_found)
            .with_state(state);
        Ok(Self {
            listener,
            local_addr,
            pool,
            router,
            game,
        })
    }

    pub fn local_addr(&self) -> SocketAddr {
        self.local_addr
    }

    pub async fn serve(
        self,
        shutdown: impl Future<Output = ()> + Send + 'static,
    ) -> Result<(), ServeError> {
        tracing::info!(
            event = "listening",
            address = %self.local_addr,
            "account service listening on loopback"
        );
        let game_task = self.game.map(game_service::PreparedGame::start);
        let game_stop = game_task.as_ref().map(game_service::GameTask::stop_signal);
        let mut result = transport::serve(self.listener, self.router, async move {
            shutdown.await;
            if let Some(stop) = game_stop {
                stop.send_replace(true);
            }
        })
        .await;
        if let Some(game) = game_task
            && let Err(error) = game.finish().await
        {
            result = Err(error);
        }
        if !database::close_pool(self.pool).await {
            result = Err(ServeError::new("pool_close_deadline"));
        }
        tracing::info!(
            event = "shutdown_complete",
            clean = result.is_ok(),
            "account service stopped"
        );
        result
    }
}

fn database_pool(config: &Config) -> PgPool {
    PgPoolOptions::new()
        // All acquisition, release and cleanup work is owned and timed, not background upkeep.
        .min_connections(0)
        .max_connections(8)
        .idle_timeout(None)
        .max_lifetime(None)
        .acquire_timeout(database::IO_TIMEOUT)
        .after_connect(|connection, _| {
            Box::pin(async move {
                sqlx::query("SET statement_timeout = '5s'")
                    .execute(&mut *connection)
                    .await?;
                sqlx::query("SET lock_timeout = '5s'")
                    .execute(connection)
                    .await?;
                Ok(())
            })
        })
        .connect_lazy_with(config.database.clone())
}

pub async fn migrate_game_ui(
    config: Config,
    expected_old_artifact: String,
) -> Result<(), StartupError> {
    let content = game_service::PreparedGame::load(&config)?
        .ok_or_else(|| StartupError::new("game_ui_migration", "game_content_required"))?;
    if content.compiled.definition().ui.is_none() {
        return Err(StartupError::new(
            "game_ui_migration",
            "ui_content_required",
        ));
    }
    let pool = database_pool(&config);
    let store = game_storage::GameStore::new(pool.clone());
    let mut lease = None;
    let result = async {
        store
            .migrate()
            .await
            .map_err(|_| StartupError::new("game_ui_migration", "schema_migration"))?;
        let acquired = store
            .acquire_world_lease(content.world_id, std::time::Duration::from_secs(30))
            .await
            .map_err(|_| StartupError::new("game_ui_migration", "exclusive_world_ownership"))?;
        lease = Some(acquired.clone());
        let engine = content.engine.clone();
        let readiness = content.readiness.clone();
        store
            .migrate_ui_content(
                &acquired,
                expected_old_artifact,
                content.artifact_hash,
                content.compiled.definition().revision.clone(),
                move |world| {
                    engine.migrate_ui_state(world)?;
                    readiness.validate_world(world)
                },
            )
            .await
            .map_err(|_| StartupError::new("game_ui_migration", "state_migration"))?;
        Ok(())
    }
    .await;
    let released = if let Some(lease) = lease {
        store.release_world_lease(&lease).await.is_ok()
    } else {
        true
    };
    let closed = database::close_pool(pool).await;
    if !released || !closed {
        return Err(StartupError::new("game_ui_migration", "cleanup"));
    }
    result
}

async fn health(State(state): State<Arc<AppState>>) -> Response {
    if let Some(game) = &state.game
        && let Err(error) = game.availability()
    {
        return error.into_response(String::new());
    }
    match database::run(&state.pool, "readiness", false, |connection| {
        Box::pin(async move {
            sqlx::query_scalar::<_, i32>("SELECT 1")
                .fetch_one(connection)
                .await
                .map_err(ApiError::database)
        })
    })
    .await
    {
        Ok(1) => Json(serde_json::json!({"status": "ready"})).into_response(),
        Ok(_) => ApiError::internal("readiness_result").into_response(String::new()),
        Err(error) => error.into_response(String::new()),
    }
}

async fn rpc(
    State(state): State<Arc<AppState>>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    request: Request<Body>,
) -> Response {
    let start = Instant::now();
    let mut request_id = String::new();
    let result = handle_rpc(&state, peer.ip(), request, &mut request_id).await;
    let (status, error_id) = match &result {
        Ok(_) => (StatusCode::OK, String::new()),
        Err(error) => (error.status, error.error_id.to_string()),
    };
    tracing::info!(
        event = "rpc_completed",
        request_id,
        error_id,
        status = status.as_u16(),
        elapsed_ms = start.elapsed().as_millis() as u64,
        "account RPC completed"
    );
    match result {
        Ok(result) => protobuf_response(StatusCode::OK, request_id, result),
        Err(error) => error.into_response(request_id),
    }
}

async fn handle_rpc(
    state: &AppState,
    peer: IpAddr,
    request: Request<Body>,
    request_id: &mut String,
) -> Result<server_message::Result, ApiError> {
    let (parts, body) = request.into_parts();
    if parts
        .headers
        .get(header::CONTENT_LENGTH)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<u64>().ok())
        .is_some_and(|length| length > MAX_REQUEST_BYTES as u64)
    {
        return Err(body_too_large());
    }
    validate_media_type(&parts.headers)?;
    let bytes = bounded_body(body).await?;
    let message = ClientMessage::decode(bytes)
        .map_err(|_| ApiError::invalid("The Protobuf request is malformed."))?;
    if message.request_id.len() == 36
        && Uuid::parse_str(&message.request_id).is_ok_and(|id| !id.is_nil())
    {
        request_id.clone_from(&message.request_id);
    }
    if matches!(
        message.command.as_ref(),
        Some(client_message::Command::Register(_) | client_message::Command::Login(_))
    ) {
        state.limiter.admit(peer)?;
    }
    validate_client_message(&message)?;
    let operation_id = Uuid::parse_str(&message.request_id)
        .map_err(|_| ApiError::invalid("A request UUID is required."))?;
    let command = message
        .command
        .ok_or_else(|| ApiError::invalid("A supported command is required."))?;
    let (operation, mutation) = match &command {
        client_message::Command::Hello(_) => ("hello", false),
        client_message::Command::Register(_) => ("registration", true),
        client_message::Command::Login(_) => ("login", true),
        client_message::Command::CurrentAccount(_) => ("current_account", false),
        client_message::Command::Logout(_) => ("logout", true),
        client_message::Command::CreateCharacter(_) => ("create_character", true),
        client_message::Command::JoinWorld(_) => ("join_world", true),
        client_message::Command::PollWorld(_) => ("poll_world", false),
        client_message::Command::WorldInput(_) => ("world_input", true),
        client_message::Command::LeaveWorld(_) => ("leave_world", true),
    };
    timeout(
        COMMAND_TIMEOUT,
        execute_command(
            state,
            &parts.headers,
            command,
            operation_id,
            &message.request_id,
        ),
    )
    .await
    .map_err(|_| ApiError::deadline(operation, "command", mutation, COMMAND_TIMEOUT))?
}

async fn execute_command(
    state: &AppState,
    headers: &HeaderMap,
    command: client_message::Command,
    operation_id: Uuid,
    request_id: &str,
) -> Result<server_message::Result, ApiError> {
    match command {
        client_message::Command::Hello(_) => {
            let mut capabilities: Vec<_> = CAPABILITIES
                .iter()
                .map(|value| (*value).to_owned())
                .collect();
            let (available, reason) = match &state.game {
                Some(game) => match game.availability() {
                    Ok(()) => (true, String::new()),
                    Err(error) => (false, error.message.to_owned()),
                },
                None => (false, GAMEPLAY_UNAVAILABLE_REASON.to_owned()),
            };
            if available {
                capabilities.push(clubscape_protocol::GAME_CAPABILITY.to_owned());
                if state.game.as_ref().is_some_and(|game| game.gameplay_ui) {
                    capabilities.push(clubscape_protocol::GAMEPLAY_UI_CAPABILITY.to_owned());
                }
            }
            Ok(server_message::Result::Hello(ServerHello {
                capabilities,
                max_request_bytes: MAX_REQUEST_BYTES as u32,
                gameplay_available: available,
                gameplay_unavailable_reason: reason,
                build_revision: state.build_revision.clone(),
            }))
        }
        client_message::Command::Register(register) => {
            let account = store::register(
                &state.pool,
                &state.passwords,
                normalize_login_name(&register.login_name)?,
                register.password,
            )
            .await?;
            Ok(server_message::Result::Registered(Registered {
                account: Some(account),
            }))
        }
        client_message::Command::Login(login) => {
            let logged_in = store::login(
                &state.pool,
                &state.passwords,
                normalize_login_name(&login.login_name)?,
                login.password,
            )
            .await?;
            Ok(server_message::Result::LoggedIn(logged_in))
        }
        client_message::Command::CurrentAccount(_) => {
            let digest = authorization_digest(headers)?;
            let (account, character_initialized) =
                store::account_snapshot(&state.pool, &digest).await?;
            let reason = if let Some(game) = &state.game {
                game.availability()
                    .err()
                    .map_or_else(String::new, |error| error.message.to_owned())
            } else if character_initialized {
                "Gameplay content is not configured.".to_owned()
            } else {
                GAMEPLAY_UNAVAILABLE_REASON.to_owned()
            };
            Ok(server_message::Result::Account(AccountSnapshot {
                account: Some(account),
                character_initialized,
                gameplay_unavailable_reason: reason,
            }))
        }
        command @ client_message::Command::Logout(_) => {
            let digest = authorization_digest(headers)?;
            if let Some(game) = &state.game {
                return game
                    .request(operation_id, request_id, digest, command)
                    .await;
            }
            store::logout(&state.pool, &digest).await?;
            Ok(server_message::Result::LoggedOut(LoggedOut {}))
        }
        command @ (client_message::Command::CreateCharacter(_)
        | client_message::Command::JoinWorld(_)
        | client_message::Command::PollWorld(_)
        | client_message::Command::WorldInput(_)
        | client_message::Command::LeaveWorld(_)) => {
            let digest = authorization_digest(headers)?;
            store::current_account(&state.pool, &digest).await?;
            if let Some(game) = &state.game {
                return game
                    .request(operation_id, request_id, digest, command)
                    .await;
            }
            Err(ApiError::new(
                StatusCode::SERVICE_UNAVAILABLE,
                ErrorCode::Unavailable,
                GAMEPLAY_UNAVAILABLE_REASON,
            ))
        }
    }
}

async fn bounded_body(body: Body) -> Result<Bytes, ApiError> {
    timeout(BODY_TIMEOUT, to_bytes(body, MAX_REQUEST_BYTES))
        .await
        .map_err(|_| ApiError::invalid("The request body timed out."))?
        .map_err(|error| {
            if error
                .source()
                .is_some_and(|source| source.is::<http_body_util::LengthLimitError>())
            {
                body_too_large()
            } else {
                ApiError::invalid("The request body could not be read.")
            }
        })
}

fn validate_media_type(headers: &HeaderMap) -> Result<(), ApiError> {
    let mut values = headers.get_all(header::CONTENT_TYPE).iter();
    let valid = values
        .next()
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.eq_ignore_ascii_case(MEDIA_TYPE))
        && values.next().is_none();
    if valid {
        Ok(())
    } else {
        Err(ApiError::new(
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            ErrorCode::InvalidArgument,
            "Content-Type must be application/x-protobuf.",
        ))
    }
}

fn body_too_large() -> ApiError {
    ApiError::new(
        StatusCode::PAYLOAD_TOO_LARGE,
        ErrorCode::InvalidArgument,
        "The request body exceeds 16 KiB.",
    )
}

fn authorization_digest(headers: &HeaderMap) -> Result<[u8; 32], ApiError> {
    let mut values = headers.get_all(header::AUTHORIZATION).iter();
    let value = values
        .next()
        .and_then(|value| value.to_str().ok())
        .ok_or_else(ApiError::unauthenticated)?;
    if values.next().is_some() {
        return Err(ApiError::unauthenticated());
    }
    let (_, token) = value
        .split_once(' ')
        .filter(|(scheme, _)| scheme.eq_ignore_ascii_case("Bearer"))
        .ok_or_else(ApiError::unauthenticated)?;
    token_digest(token).ok_or_else(ApiError::unauthenticated)
}

async fn method_not_allowed() -> Response {
    ApiError::new(
        StatusCode::METHOD_NOT_ALLOWED,
        ErrorCode::InvalidArgument,
        "Use POST for the account RPC endpoint.",
    )
    .into_response(String::new())
}

async fn web_or_not_found(State(state): State<Arc<AppState>>, request: Request<Body>) -> Response {
    if let Some(game) = &state.game
        && let Some(response) =
            game.assets
                .response(request.uri().path(), request.method(), request.headers())
    {
        return response;
    }
    if let Some(assets) = &state.web_assets
        && let Some(response) =
            assets.response(request.uri().path(), request.method(), request.headers())
    {
        return response;
    }
    ApiError::new(
        StatusCode::NOT_FOUND,
        ErrorCode::InvalidArgument,
        "The requested endpoint does not exist.",
    )
    .into_response(String::new())
}

#[cfg(test)]
mod tests {
    use axum::http::HeaderValue;

    use super::*;

    #[test]
    fn authorization_requires_one_well_formed_canonical_bearer_token() {
        let token = crypto::SessionToken::generate().unwrap();
        for scheme in ["Bearer", "bearer", "BEARER"] {
            let mut headers = HeaderMap::new();
            headers.insert(
                header::AUTHORIZATION,
                HeaderValue::from_str(&format!("{scheme} {}", token.raw)).unwrap(),
            );
            assert_eq!(authorization_digest(&headers).unwrap(), token.digest);
            headers.append(
                header::AUTHORIZATION,
                HeaderValue::from_str(&format!("Bearer {}", token.raw)).unwrap(),
            );
            assert!(authorization_digest(&headers).is_err());
        }
        for value in [
            "",
            "Bearer",
            "Bearer ",
            "Basic invalid",
            "Bearer invalid",
            &format!("Bearer  {}", token.raw),
            &format!("Bearer {} ", token.raw),
        ] {
            let mut headers = HeaderMap::new();
            headers.insert(header::AUTHORIZATION, HeaderValue::from_str(value).unwrap());
            assert!(authorization_digest(&headers).is_err());
        }
        let mut headers = HeaderMap::new();
        assert!(authorization_digest(&headers).is_err());
        headers.insert(
            header::AUTHORIZATION,
            HeaderValue::from_bytes(b"Bearer \xff").unwrap(),
        );
        assert!(authorization_digest(&headers).is_err());
    }

    #[test]
    fn media_type_is_explicit_and_has_no_ambiguous_duplicates() {
        let mut headers = HeaderMap::new();
        assert!(validate_media_type(&headers).is_err());
        headers.insert(header::CONTENT_TYPE, HeaderValue::from_static(MEDIA_TYPE));
        assert!(validate_media_type(&headers).is_ok());
        headers.append(header::CONTENT_TYPE, HeaderValue::from_static(MEDIA_TYPE));
        assert!(validate_media_type(&headers).is_err());
        headers.insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/json"),
        );
        assert!(validate_media_type(&headers).is_err());
    }

    #[tokio::test]
    async fn streamed_body_limits_and_transport_errors_are_bounded_protocol_failures() {
        let exact = bounded_body(Body::from(vec![0_u8; MAX_REQUEST_BYTES]))
            .await
            .unwrap();
        assert_eq!(exact.len(), MAX_REQUEST_BYTES);
        let chunks = futures_util::stream::iter([
            Ok::<_, std::io::Error>(vec![0_u8; MAX_REQUEST_BYTES]),
            Ok(vec![0_u8; 1]),
        ]);
        let oversized = bounded_body(Body::from_stream(chunks)).await.unwrap_err();
        assert_eq!(oversized.status, StatusCode::PAYLOAD_TOO_LARGE);
        assert_eq!(oversized.code, ErrorCode::InvalidArgument);
        let failed = futures_util::stream::iter([Err::<Vec<u8>, _>(std::io::Error::new(
            std::io::ErrorKind::ConnectionReset,
            "never echo this body transport detail",
        ))]);
        let error = bounded_body(Body::from_stream(failed)).await.unwrap_err();
        assert_eq!(error.status, StatusCode::BAD_REQUEST);
        assert!(!error.message.contains("transport detail"));
    }
}

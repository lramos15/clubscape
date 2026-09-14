use clubscape_protocol::{Account, LoggedIn};
use sqlx::{Connection as _, PgPool};
use uuid::Uuid;

use crate::{
    crypto::{Passwords, SessionToken},
    database,
    error::ApiError,
};

#[derive(sqlx::FromRow)]
struct StoredAccount {
    account_id: Uuid,
    login_name: String,
    password_hash: String,
}

#[derive(sqlx::FromRow)]
struct Identity {
    account_id: Uuid,
    login_name: String,
}

impl From<Identity> for Account {
    fn from(identity: Identity) -> Self {
        Self {
            account_id: identity.account_id.to_string(),
            login_name: identity.login_name,
        }
    }
}

pub(crate) async fn register(
    pool: &PgPool,
    passwords: &Passwords,
    login_name: String,
    password: String,
) -> Result<Account, ApiError> {
    let password_hash = passwords.hash(password).await?;
    let account_id = Uuid::new_v4();
    database::run(pool, "registration", true, move |connection| {
        Box::pin(async move {
            let result = sqlx::query(
                "INSERT INTO accounts (account_id, login_name, password_hash) VALUES ($1, $2, $3)",
            )
            .bind(account_id)
            .bind(&login_name)
            .bind(password_hash)
            .execute(connection)
            .await;
            match result {
                Ok(_) => Ok(Account {
                    account_id: account_id.to_string(),
                    login_name,
                }),
                Err(error)
                    if error.as_database_error().is_some_and(|database| {
                        database.is_unique_violation()
                            && database.constraint() == Some("accounts_login_name_key")
                    }) =>
                {
                    Err(ApiError::new(
                        axum::http::StatusCode::CONFLICT,
                        clubscape_protocol::ErrorCode::Conflict,
                        "That login name is already registered.",
                    ))
                }
                Err(error) => Err(ApiError::database(error)),
            }
        })
    })
    .await
}

pub(crate) async fn login(
    pool: &PgPool,
    passwords: &Passwords,
    login_name: String,
    password: String,
) -> Result<LoggedIn, ApiError> {
    let account = database::run(pool, "login_lookup", false, move |connection| {
        Box::pin(async move {
            sqlx::query_as::<_, StoredAccount>(
                "SELECT account_id, login_name, password_hash FROM accounts WHERE login_name = $1",
            )
            .bind(login_name)
            .fetch_optional(connection)
            .await
            .map_err(ApiError::database)
        })
    })
    .await?;
    let hash = account
        .as_ref()
        .map(|account| account.password_hash.clone());
    let verified = passwords.verify(password, hash).await?;
    let account = account
        .filter(|_| verified)
        .ok_or_else(ApiError::unauthenticated)?;
    let token = SessionToken::generate()?;
    database::run(pool, "login_issuance", true, move |connection| {
        Box::pin(async move {
            let mut transaction = connection.begin().await.map_err(ApiError::database)?;
            // The whole locked transaction, including commit, shares one client-side deadline.
            let locked = sqlx::query_scalar::<_, Uuid>(
                "SELECT account_id FROM accounts WHERE account_id = $1 FOR UPDATE",
            )
            .bind(account.account_id)
            .fetch_optional(&mut *transaction)
            .await
            .map_err(ApiError::database)?;
            if locked.is_none() {
                return Err(ApiError::unauthenticated());
            }
            sqlx::query(
                "DELETE FROM account_sessions WHERE account_id = $1 AND expires_at <= clock_timestamp()",
            )
            .bind(account.account_id)
            .execute(&mut *transaction)
            .await
            .map_err(ApiError::database)?;
            sqlx::query(
                "DELETE FROM account_sessions
                 WHERE account_id = $1 AND token_digest IN (
                     SELECT token_digest FROM account_sessions WHERE account_id = $1
                     ORDER BY created_at DESC, token_digest DESC OFFSET 4
                 )",
            )
            .bind(account.account_id)
            .execute(&mut *transaction)
            .await
            .map_err(ApiError::database)?;
            let expires_at_unix_ms = sqlx::query_scalar::<_, i64>(
                "WITH stamp AS (SELECT clock_timestamp() AS issued_at)
                 INSERT INTO account_sessions (token_digest, account_id, created_at, expires_at)
                 SELECT $1, $2, issued_at, issued_at + INTERVAL '30 minutes' FROM stamp
                 RETURNING floor(extract(epoch FROM expires_at) * 1000)::bigint",
            )
            .bind(token.digest.as_slice())
            .bind(account.account_id)
            .fetch_one(&mut *transaction)
            .await
            .map_err(ApiError::database)?;
            transaction.commit().await.map_err(ApiError::database)?;
            Ok(LoggedIn {
                account: Some(Account {
                    account_id: account.account_id.to_string(),
                    login_name: account.login_name,
                }),
                session_token: token.raw,
                expires_at_unix_ms,
            })
        })
    })
    .await
}

pub(crate) async fn current_account(pool: &PgPool, digest: &[u8; 32]) -> Result<Account, ApiError> {
    let digest = *digest;
    database::run(pool, "current_account", false, move |connection| {
        Box::pin(async move {
            let account = sqlx::query_as::<_, Identity>(
                "SELECT a.account_id, a.login_name FROM accounts a
                 JOIN account_sessions s ON s.account_id = a.account_id
                 WHERE s.token_digest = $1 AND s.expires_at > clock_timestamp()",
            )
            .bind(digest.as_slice())
            .fetch_optional(connection)
            .await
            .map_err(ApiError::database)?;
            account
                .map(Account::from)
                .ok_or_else(ApiError::unauthenticated)
        })
    })
    .await
}

pub(crate) async fn account_snapshot(
    pool: &PgPool,
    digest: &[u8; 32],
) -> Result<(Account, bool), ApiError> {
    let digest = *digest;
    database::run(pool, "current_account", false, move |connection| {
        Box::pin(async move {
            let account: Option<(Uuid, String, bool)> = sqlx::query_as(
                "SELECT a.account_id, a.login_name,
                 EXISTS (SELECT 1 FROM game_characters c WHERE c.account_id = a.account_id)
             FROM accounts a JOIN account_sessions s ON s.account_id = a.account_id
             WHERE s.token_digest = $1 AND s.expires_at > clock_timestamp()",
            )
            .bind(digest.as_slice())
            .fetch_optional(connection)
            .await
            .map_err(ApiError::database)?;
            let (id, login_name, initialized) = account.ok_or_else(ApiError::unauthenticated)?;
            Ok((
                Account {
                    account_id: id.to_string(),
                    login_name,
                },
                initialized,
            ))
        })
    })
    .await
}

pub(crate) async fn logout(pool: &PgPool, digest: &[u8; 32]) -> Result<(), ApiError> {
    let digest = *digest;
    database::run(pool, "logout", true, move |connection| {
        Box::pin(async move {
            let deleted = sqlx::query(
                "DELETE FROM account_sessions
                 WHERE token_digest = $1 AND expires_at > clock_timestamp()",
            )
            .bind(digest.as_slice())
            .execute(connection)
            .await
            .map_err(ApiError::database)?;
            if deleted.rows_affected() == 1 {
                Ok(())
            } else {
                Err(ApiError::unauthenticated())
            }
        })
    })
    .await
}

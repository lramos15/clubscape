use std::{
    future::Future,
    ops::{Deref, DerefMut},
    pin::Pin,
    time::Duration,
};

use sqlx::{PgConnection, PgPool, Postgres, pool::PoolConnection};
use tokio::time::timeout;

use crate::error::ApiError;

pub(crate) const IO_TIMEOUT: Duration = Duration::from_secs(5);
pub(crate) const MIGRATION_TIMEOUT: Duration = Duration::from_secs(5);
pub(crate) const CLEANUP_TIMEOUT: Duration = Duration::from_secs(2);

pub(crate) struct Connection {
    inner: Option<PoolConnection<Postgres>>,
}

impl Connection {
    pub(crate) fn new(connection: PoolConnection<Postgres>) -> Self {
        Self {
            inner: Some(connection),
        }
    }

    pub(crate) async fn release(mut self) -> bool {
        let mut connection = self
            .inner
            .take()
            .expect("connection is owned until release");
        // SQLx's default Drop spawns an unbounded release ping. Own and bound that future instead.
        timeout(CLEANUP_TIMEOUT, connection.return_to_pool())
            .await
            .is_ok()
    }
}

impl Deref for Connection {
    type Target = PgConnection;

    fn deref(&self) -> &Self::Target {
        self.inner
            .as_ref()
            .expect("connection has not been released")
    }
}

impl DerefMut for Connection {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.inner
            .as_mut()
            .expect("connection has not been released")
    }
}

impl Drop for Connection {
    fn drop(&mut self) {
        if let Some(connection) = self.inner.take() {
            // A cancelled/failed operation must not leave a response/rollback waiter in the pool.
            drop(connection.detach());
        }
    }
}

pub(crate) async fn run<T, F>(
    pool: &PgPool,
    operation: &'static str,
    mutation: bool,
    execute: F,
) -> Result<T, ApiError>
where
    T: Send,
    F: for<'connection> FnOnce(
            &'connection mut PgConnection,
        ) -> Pin<
            Box<dyn Future<Output = Result<T, ApiError>> + Send + 'connection>,
        > + Send,
{
    deadline(operation, mutation, async {
        let mut connection = Connection::new(pool.acquire().await.map_err(ApiError::database)?);
        let result = execute(&mut connection).await?;
        if !connection.release().await {
            return Err(ApiError::deadline(
                operation,
                "database_release",
                mutation,
                CLEANUP_TIMEOUT,
            ));
        }
        Ok(result)
    })
    .await
}

async fn deadline<T>(
    operation: &'static str,
    mutation: bool,
    future: impl Future<Output = Result<T, ApiError>>,
) -> Result<T, ApiError> {
    timeout(IO_TIMEOUT, future)
        .await
        .map_err(|_| ApiError::deadline(operation, "database_operation", mutation, IO_TIMEOUT))?
}

pub(crate) async fn close_pool(pool: PgPool) -> bool {
    timeout(CLEANUP_TIMEOUT, pool.close()).await.is_ok()
}

#[cfg(test)]
mod tests {
    use std::future::pending;

    use axum::http::StatusCode;
    use clubscape_protocol::ErrorCode;

    use super::*;

    #[tokio::test(start_paused = true)]
    async fn client_deadlines_do_not_depend_on_database_or_acquisition_timeouts() {
        let start = tokio::time::Instant::now();
        let error = deadline("readiness", false, pending::<Result<(), ApiError>>())
            .await
            .unwrap_err();
        assert_eq!(start.elapsed(), IO_TIMEOUT);
        assert_eq!(error.status, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(error.code, ErrorCode::Unavailable);
        assert_eq!(error.retry_after_seconds, 0);
        assert!(!error.error_id.is_nil());
    }

    #[tokio::test(start_paused = true)]
    async fn a_timed_out_write_is_not_retried_or_reported_as_rolled_back() {
        let attempts = std::sync::atomic::AtomicUsize::new(0);
        let error = deadline("registration", true, async {
            attempts.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            pending::<Result<(), ApiError>>().await
        })
        .await
        .unwrap_err();
        assert_eq!(attempts.load(std::sync::atomic::Ordering::SeqCst), 1);
        assert!(error.message.contains("unknown"));
        assert!(error.message.contains("Do not automatically retry"));
        assert_eq!(error.retry_after_seconds, 0);
    }
}

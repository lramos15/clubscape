use std::{future::Future, io, time::Duration};

use axum::{Extension, Router, extract::ConnectInfo};
use hyper::server::conn::http1;
use hyper_util::{rt::TokioIo, service::TowerToHyperService};
use tokio::{
    net::TcpListener,
    sync::watch,
    task::{JoinSet, yield_now},
    time::{sleep, timeout},
};

use crate::ServeError;

const DRAIN_TIMEOUT: Duration = Duration::from_secs(6);
const ABORT_TIMEOUT: Duration = Duration::from_secs(1);
const ACCEPT_BACKOFF: Duration = Duration::from_secs(1);

async fn accept_with_retry<F, A, T>(mut accept: A) -> T
where
    A: FnMut() -> F,
    F: Future<Output = io::Result<T>>,
{
    loop {
        match accept().await {
            Ok(connection) => return connection,
            Err(error) => {
                let transient = matches!(
                    error.kind(),
                    io::ErrorKind::ConnectionRefused
                        | io::ErrorKind::ConnectionAborted
                        | io::ErrorKind::ConnectionReset
                        | io::ErrorKind::Interrupted
                );
                tracing::warn!(
                    event = "listener_accept_retry",
                    error_kind = ?error.kind(),
                    os_error = error.raw_os_error(),
                    retry_after_ms = if transient { 0 } else { ACCEPT_BACKOFF.as_millis() },
                    "TCP accept failed; listener remains available for retry"
                );
                if transient {
                    yield_now().await;
                } else {
                    sleep(ACCEPT_BACKOFF).await;
                }
            }
        }
    }
}

pub(crate) async fn serve(
    listener: TcpListener,
    router: Router,
    shutdown: impl Future<Output = ()> + Send,
) -> Result<(), ServeError> {
    let mut connections = JoinSet::new();
    let (draining, _) = watch::channel(());
    tokio::pin!(shutdown);
    let mut failure = None;

    loop {
        tokio::select! {
            biased;
            _ = &mut shutdown => break,
            result = connections.join_next(), if !connections.is_empty() => {
                if result.is_some_and(|result| result.is_err()) {
                    failure = Some(ServeError::new("connection_worker"));
                    break;
                }
            },
            (socket, peer) = accept_with_retry(|| listener.accept()) => {
                let service = TowerToHyperService::new(
                    router.clone().layer(Extension(ConnectInfo(peer))),
                );
                let mut draining = draining.subscribe();
                // Own every HTTP task: timing out axum::serve alone leaves its tasks detached.
                connections.spawn(async move {
                    let builder = http1::Builder::new();
                    let connection = builder.serve_connection(TokioIo::new(socket), service);
                    tokio::pin!(connection);
                    tokio::select! {
                        _ = &mut connection => {},
                        _ = draining.changed() => {
                            connection.as_mut().graceful_shutdown();
                            let _ = connection.await;
                        }
                    }
                });
            },
        }
    }
    drop(listener);
    drop(router);
    tracing::info!(event = "shutdown_started", "account service shutting down");
    draining.send_replace(());
    if timeout(DRAIN_TIMEOUT, async {
        while connections.join_next().await.is_some() {}
    })
    .await
    .is_err()
    {
        failure = Some(ServeError::new("shutdown_drain_deadline"));
        connections.abort_all();
        if timeout(ABORT_TIMEOUT, async {
            while connections.join_next().await.is_some() {}
        })
        .await
        .is_err()
        {
            failure = Some(ServeError::new("shutdown_abort_deadline"));
        }
    }
    drop(connections);
    failure.map_or(Ok(()), Err)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{collections::VecDeque, future::ready};
    use tokio::{sync::oneshot, time::Instant};

    #[tokio::test(start_paused = true)]
    async fn transient_errors_retry_without_terminating_the_listener() {
        let mut outcomes = VecDeque::from([
            Err(io::ErrorKind::ConnectionRefused.into()),
            Err(io::ErrorKind::ConnectionAborted.into()),
            Err(io::ErrorKind::ConnectionReset.into()),
            Err(io::ErrorKind::Interrupted.into()),
            Ok(42),
        ]);
        let start = Instant::now();
        assert_eq!(
            accept_with_retry(|| ready(outcomes.pop_front().unwrap())).await,
            42
        );
        assert!(outcomes.is_empty());
        assert_eq!(start.elapsed(), Duration::ZERO);
    }

    #[tokio::test(start_paused = true)]
    async fn resource_errors_back_off_before_retrying() {
        let mut outcomes = VecDeque::from([
            Err(io::Error::other("descriptor exhaustion fixture")),
            Err(io::Error::other("buffer exhaustion fixture")),
            Ok(42),
        ]);
        let start = Instant::now();
        assert_eq!(
            accept_with_retry(|| ready(outcomes.pop_front().unwrap())).await,
            42
        );
        assert_eq!(start.elapsed(), ACCEPT_BACKOFF * 2);
    }

    #[tokio::test(start_paused = true)]
    async fn shutdown_interrupts_both_transient_retry_and_resource_backoff() {
        for kind in [io::ErrorKind::ConnectionAborted, io::ErrorKind::Other] {
            let (shutdown, received) = oneshot::channel();
            let server = tokio::spawn(async move {
                tokio::select! {
                    biased;
                    _ = received => {},
                    _ = accept_with_retry(|| ready(Err::<(), _>(io::Error::from(kind)))) => {
                        panic!("an always-failing listener cannot produce a connection");
                    },
                }
            });
            yield_now().await;
            shutdown.send(()).unwrap();
            timeout(Duration::from_millis(1), server)
                .await
                .expect("shutdown must not wait for the accept backoff")
                .unwrap();
        }
    }
}

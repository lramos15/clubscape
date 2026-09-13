use std::{future::Future, time::Duration};

use axum::{Extension, Router, extract::ConnectInfo};
use hyper::server::conn::http1;
use hyper_util::{rt::TokioIo, service::TowerToHyperService};
use tokio::{net::TcpListener, sync::watch, task::JoinSet, time::timeout};

use crate::ServeError;

const DRAIN_TIMEOUT: Duration = Duration::from_secs(6);
const ABORT_TIMEOUT: Duration = Duration::from_secs(1);

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
            accepted = listener.accept() => {
                let (socket, peer) = match accepted {
                    Ok(connection) => connection,
                    Err(_) => {
                        failure = Some(ServeError::new("listener_accept"));
                        break;
                    }
                };
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

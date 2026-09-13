use std::process::ExitCode;

use clubscape_server::{Config, Service};
use tracing_subscriber::EnvFilter;
use uuid::Uuid;

#[tokio::main]
async fn main() -> ExitCode {
    tracing_subscriber::fmt()
        .json()
        .with_env_filter(EnvFilter::new("off,clubscape_server=info"))
        .with_current_span(false)
        .with_span_list(false)
        .init();

    let config = match Config::from_env() {
        Ok(config) => config,
        Err(error) => {
            tracing::error!(
                event = "configuration_failure",
                error_id = %Uuid::new_v4(),
                error_kind = %error,
                "invalid server configuration"
            );
            return ExitCode::FAILURE;
        }
    };
    #[cfg(unix)]
    let (mut interrupt, mut terminate) = {
        use tokio::signal::unix::{SignalKind, signal};
        match (
            signal(SignalKind::interrupt()),
            signal(SignalKind::terminate()),
        ) {
            (Ok(interrupt), Ok(terminate)) => (interrupt, terminate),
            _ => {
                tracing::error!(
                    event = "signal_failure",
                    error_id = %Uuid::new_v4(),
                    error_kind = "signal_registration",
                    "could not register shutdown signals"
                );
                return ExitCode::FAILURE;
            }
        }
    };
    let service = match Service::bind(config).await {
        Ok(service) => service,
        Err(_) => return ExitCode::FAILURE,
    };
    let shutdown = async move {
        #[cfg(unix)]
        tokio::select! {
            _ = interrupt.recv() => {},
            _ = terminate.recv() => {},
        }
        #[cfg(not(unix))]
        if tokio::signal::ctrl_c().await.is_err() {
            tracing::error!(
                event = "signal_failure",
                error_id = %Uuid::new_v4(),
                error_kind = "signal_listener",
                "shutdown signal listener failed"
            );
        }
    };
    match service.serve(shutdown).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(_) => ExitCode::FAILURE,
    }
}

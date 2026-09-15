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
    let arguments: Vec<_> = std::env::args().skip(1).collect();
    if !arguments.is_empty() {
        if arguments.len() == 3 && arguments[0] == "migrate-ui" && arguments[1] == "--from" {
            return match clubscape_server::migrate_game_ui(config, arguments[2].clone()).await {
                Ok(()) => {
                    tracing::info!(
                        event = "game_ui_migration_complete",
                        "explicit UI metadata/content migration committed"
                    );
                    ExitCode::SUCCESS
                }
                Err(_) => ExitCode::FAILURE,
            };
        }
        tracing::error!(
            event = "argument_error",
            "use no arguments to serve, or migrate-ui --from <old-artifact-sha256>"
        );
        return ExitCode::FAILURE;
    }
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

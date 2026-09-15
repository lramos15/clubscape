//! Test-process ownership only. All account/game HTTP, storage, ticks and mechanics
//! are the unchanged production Service; there are no test callbacks or fixtures.

use std::{env, process::ExitCode};

use anyhow::{Context, Result};
use clubscape_server::{Config, Service};
use tracing_subscriber::EnvFilter;

async fn serve() -> Result<()> {
    let root = env::var("CLUBSCAPE_GAME_ROOT").context("A real product GameRoot is required")?;
    let config = Config::from_env()?.with_game_root(root)?;
    let mut terminate = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;
    let mut interrupt = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::interrupt())?;
    let service = Service::bind(config).await?;
    service
        .serve(async move {
            tokio::select! {
                _ = terminate.recv() => {},
                _ = interrupt.recv() => {},
            }
        })
        .await?;
    Ok(())
}

#[tokio::main]
async fn main() -> ExitCode {
    tracing_subscriber::fmt()
        .json()
        .with_env_filter(EnvFilter::new("off,clubscape_server=info"))
        .with_current_span(false)
        .with_span_list(false)
        .init();
    match serve().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("Real Service/public Config launcher failed: {error}");
            ExitCode::FAILURE
        }
    }
}

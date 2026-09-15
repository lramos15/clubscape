//! Test-process ownership only. All account/game HTTP, storage, ticks and mechanics
//! are the unchanged production Service; there are no test callbacks or fixtures.

use std::{env, io::Read, path::Path, process::ExitCode};

use anyhow::{Context, Result, ensure};
use clubscape_content::{ValidationMode, load_compiled};
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

fn inspect_artifact(path: &Path) -> Result<()> {
    let canonical = path.canonicalize()?;
    ensure!(
        canonical.starts_with(env::current_dir()?.canonicalize()?),
        "Artifact inspection must stay inside this worktree"
    );
    ensure!(
        canonical.metadata()?.is_file(),
        "Artifact must be a regular file"
    );
    let mut bytes = Vec::new();
    std::fs::File::open(canonical)?
        .take(clubscape_content::MAX_INPUT_BYTES as u64 + 1)
        .read_to_end(&mut bytes)?;
    ensure!(
        bytes.len() <= clubscape_content::MAX_INPUT_BYTES,
        "Artifact exceeds strict compiler input bound"
    );
    let compiled = load_compiled(&bytes, ValidationMode::Runtime)?;
    println!(
        "{}",
        serde_json::json!({
            "schema_version": 1,
            "content_revision": compiled.definition().revision,
            "content_schema_version": compiled.definition().schema_version,
            "artifact_sha256": clubscape_content::sha256(&bytes),
            "referenced_assets": compiled.referenced_assets().iter().map(ToString::to_string).collect::<Vec<_>>(),
            "unresolved_bindings": compiled.report().unresolved_bindings,
            "gameplay_verified": false
        })
    );
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
    let arguments: Vec<_> = env::args_os().skip(1).collect();
    let result = if arguments.len() == 2 && arguments[0] == "inspect-artifact" {
        inspect_artifact(Path::new(&arguments[1]))
    } else if arguments.is_empty() {
        serve().await
    } else {
        Err(anyhow::anyhow!(
            "Expected no arguments, or inspect-artifact <project-artifact>"
        ))
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("Real Service/public Config launcher failed: {error}");
            ExitCode::FAILURE
        }
    }
}

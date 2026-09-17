use std::{collections::BTreeMap, env, process::ExitCode};

use clubscape_content::{ValidationMode, load_compiled, sha256};
use clubscape_server::{Config, Service};
use tracing_subscriber::EnvFilter;

fn inspect_artifact(path: &str) -> Result<(), Box<dyn std::error::Error>> {
    let bytes = std::fs::read(path)?;
    let compiled = load_compiled(&bytes, ValidationMode::Runtime)?;
    let smallest_assets: BTreeMap<_, _> = compiled
        .referenced_assets()
        .iter()
        .map(|asset| (asset.as_str(), "/assets/"))
        .collect();
    let minimum = serde_json::to_vec(&smallest_assets)?.len();
    println!(
        "{}",
        serde_json::json!({
            "strict_compiler_load": true,
            "artifact_sha256": sha256(&bytes),
            "referenced_assets_from_actual_compiler": smallest_assets.len(),
            "minimum_assets_map_bytes": minimum,
            "compatibility_verified": false
        })
    );
    Ok(())
}

// Only the public configuration constructor differs from the regular server binary.
// All loading, validation, authentication, ticks, storage, and gameplay remain in Service.
#[tokio::main(worker_threads = 2)]
async fn main() -> ExitCode {
    let arguments: Vec<_> = env::args().collect();
    if arguments.len() == 3 && arguments[1] == "--inspect-artifact" {
        return match inspect_artifact(&arguments[2]) {
            Ok(()) => ExitCode::SUCCESS,
            Err(error) => {
                eprintln!("Strict compiled artifact inspection failed: {error}");
                ExitCode::FAILURE
            }
        };
    }
    tracing_subscriber::fmt()
        .json()
        .with_env_filter(EnvFilter::new("off,clubscape_server=info"))
        .with_current_span(false)
        .with_span_list(false)
        .init();

    let config = (|| {
        let database = env::var("DATABASE_URL").map_err(|_| "missing_database")?;
        let bind = env::var("CLUBSCAPE_BIND").map_err(|_| "missing_loopback_bind")?;
        let revision =
            env::var("CLUBSCAPE_BUILD_REVISION").map_err(|_| "missing_build_revision")?;
        let game = env::var("CLUBSCAPE_GAME_ROOT").map_err(|_| "missing_game_root")?;
        Config::new(&database, &bind, Some(&revision))
            .and_then(|config| config.with_game_root(game))
            .map_err(|_| "invalid_strict_configuration")
    })();
    let Ok(config) = config else {
        eprintln!("Strict service probe configuration rejected");
        return ExitCode::FAILURE;
    };
    let service = match Service::bind(config).await {
        Ok(service) => service,
        Err(error) => {
            eprintln!("Unchanged authoritative Service::bind rejected the source pack: {error}");
            return ExitCode::FAILURE;
        }
    };
    let Ok(mut terminate) =
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
    else {
        eprintln!("Could not own a shutdown signal");
        return ExitCode::FAILURE;
    };
    match service
        .serve(async move {
            tokio::select! {
                _ = terminate.recv() => {},
                _ = tokio::signal::ctrl_c() => {},
            }
        })
        .await
    {
        Ok(()) => ExitCode::SUCCESS,
        Err(_) => ExitCode::FAILURE,
    }
}

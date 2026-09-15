use std::{
    collections::BTreeMap,
    fs,
    io::Read,
    path::{Component, Path},
    sync::Arc,
};

use clubscape_content::{CompiledContent, ValidationMode, load_compiled, sha256};
use clubscape_game_types::AssetId;
use clubscape_world_engine::WorldEngine;
use serde::Deserialize;
use uuid::Uuid;

use super::readiness::{Profile, Readiness};
use crate::{Config, StartupError, web_assets::WebAssets};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    schema_version: u32,
    world_id: Uuid,
    artifact: String,
    sha256: String,
    content_manifest_path: String,
    assets: BTreeMap<AssetId, String>,
    #[serde(default)]
    readiness_profile: Option<Profile>,
}

pub(crate) struct LoadedContent {
    pub world_id: Uuid,
    pub artifact_hash: String,
    pub compiled: Arc<CompiledContent>,
    pub engine: Arc<WorldEngine>,
    pub assets: Arc<WebAssets>,
    pub public_manifest: String,
    pub readiness: Arc<Readiness>,
}

pub(super) fn load(config: &Config) -> Result<Option<LoadedContent>, StartupError> {
    let Some(root) = config.game_root.as_deref() else {
        return Ok(None);
    };
    let root = root.canonicalize().map_err(|_| failure("game_root"))?;
    let descriptor = read_file(&root, "clubscape-game.json", 256 * 1024)?;
    let manifest: Manifest =
        serde_json::from_slice(&descriptor).map_err(|_| failure("game_manifest"))?;
    if manifest.schema_version != 1
        || manifest.world_id.is_nil()
        || manifest.sha256.len() != 64
        || !manifest
            .sha256
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        || !manifest.content_manifest_path.starts_with("/content/")
        || manifest.assets.len() > 20_000
    {
        return Err(failure("game_manifest_identity"));
    }
    let artifact = read_file(
        &root,
        &manifest.artifact,
        clubscape_content::MAX_INPUT_BYTES,
    )?;
    if sha256(&artifact) != manifest.sha256 {
        return Err(failure("game_artifact_hash"));
    }
    let mode = ValidationMode::Runtime;
    #[cfg(test)]
    let mode = if config.game_test_fixture {
        ValidationMode::TestFixture
    } else {
        mode
    };
    let compiled = load_compiled(&artifact, mode).map_err(|error| {
        tracing::error!(event = "game_content_validation", code = ?error.code,
            "configured compiled artifact failed strict validation");
        failure("game_artifact_validation")
    })?;
    let readiness = Readiness::check(&compiled, manifest.readiness_profile.as_ref())
        .map_err(|_| {
            tracing::error!(event = "game_unresolved_bindings", paths = ?compiled.report().unresolved_bindings,
                "configured runtime has unresolved required source bindings");
            failure("game_required_bindings")
        })?;
    tracing::info!(event = "game_readiness", profile = ?readiness.profile, inactive = ?readiness.inactive,
        "required source bindings checked; inactivity proofs are not milestone acceptance");
    let assets = WebAssets::load_game(&root).map_err(|_| failure("game_assets"))?;
    if !assets.contains(&manifest.content_manifest_path)
        || compiled.referenced_assets().iter().any(|id| {
            manifest
                .assets
                .get(id)
                .is_none_or(|url| !assets.contains(url))
        })
        || manifest.assets.values().any(|url| !assets.contains(url))
    {
        return Err(failure("game_asset_membership"));
    }
    let engine = WorldEngine::new(Arc::new(compiled.definition().clone()))
        .map_err(|_| failure("game_engine_initialization"))?;
    Ok(Some(LoadedContent {
        world_id: manifest.world_id,
        artifact_hash: manifest.sha256,
        compiled: Arc::new(compiled),
        engine: Arc::new(engine),
        assets: Arc::new(assets),
        public_manifest: manifest.content_manifest_path,
        readiness: Arc::new(readiness),
    }))
}

fn read_file(root: &Path, relative: &str, maximum: usize) -> Result<Vec<u8>, StartupError> {
    if relative.is_empty()
        || relative.len() > 256
        || relative.contains('\\')
        || !Path::new(relative)
            .components()
            .all(|part| matches!(part, Component::Normal(_)))
    {
        return Err(failure("game_file_path"));
    }
    let mut path = root.to_path_buf();
    for part in Path::new(relative).components() {
        path.push(part);
        if fs::symlink_metadata(&path)
            .map_err(|_| failure("game_file_metadata"))?
            .file_type()
            .is_symlink()
        {
            return Err(failure("game_file_symlink"));
        }
    }
    let metadata = fs::metadata(&path).map_err(|_| failure("game_file_metadata"))?;
    if !metadata.is_file() || metadata.len() > maximum as u64 {
        return Err(failure("game_file_size"));
    }
    let mut bytes = Vec::new();
    fs::File::open(path)
        .map_err(|_| failure("game_file_open"))?
        .take(maximum as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| failure("game_file_read"))?;
    if bytes.len() > maximum || bytes.len() as u64 != metadata.len() {
        return Err(failure("game_file_changed"));
    }
    Ok(bytes)
}

fn failure(kind: &'static str) -> StartupError {
    StartupError::new("game_content", kind)
}

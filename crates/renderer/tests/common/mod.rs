//! Shared test-input access. Published buffers are read first (raw file, else its published
//! gzip twin inflated on the fly); a declared input that is missing fails the test loudly
//! instead of letting it pass with the case silently skipped.
#![allow(dead_code)]

use std::io::Read;
use std::path::{Path, PathBuf};

pub fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn inflate(bytes: &[u8], what: &str) -> Vec<u8> {
    let mut out = Vec::new();
    flate2::read::GzDecoder::new(bytes)
        .read_to_end(&mut out)
        .unwrap_or_else(|e| panic!("{what}: gzip twin is corrupt: {e}"));
    out
}

/// Reads a render asset by manifest key (`scenes/<name>.bin`, `models/...`, `anim/...`). Raw
/// scene buffers are published only as `*.gz` twins, which are inflated here so a clean
/// checkout exercises the same bytes the adapter fetches.
pub fn read_asset(key: &str) -> Vec<u8> {
    let path = repo_root().join("assets/compiled/render").join(key);
    if let Ok(bytes) = std::fs::read(&path) {
        return bytes;
    }
    let twin = PathBuf::from(format!("{}.gz", path.display()));
    match std::fs::read(&twin) {
        Ok(bytes) => inflate(&bytes, key),
        Err(_) => panic!(
            "render input {key} is missing: neither {} nor its published gzip twin {} exists \
             (the manifest declares it; see tools/render-assets/README.md)",
            path.display(),
            twin.display()
        ),
    }
}

/// Whether a render asset (raw or gzip twin) exists.
pub fn asset_available(key: &str) -> bool {
    let path = repo_root().join("assets/compiled/render").join(key);
    path.exists() || PathBuf::from(format!("{}.gz", path.display())).exists()
}

/// Reads a reproducible local export that is deliberately not published (world blocks, pinned
/// validation twins, baked frames). Tests that depend on these are `#[ignore]`d with the
/// reproduction command; when run, a missing file is a failure.
pub fn read_local_export(key: &str, reproduce: &str) -> Vec<u8> {
    let path = repo_root().join("assets/compiled/render").join(key);
    std::fs::read(&path).unwrap_or_else(|e| {
        panic!(
            "local render export {key} is missing ({e}); reproduce it with `{reproduce}` \
             (this test is ignored by default for that reason)"
        )
    })
}

/// The approved NPC captures: (npc, sequence, frame count, camera y, camera z) exactly as the
/// reference pack recorded them (goblin 3028 idle/walk, penguin 2063 idle/walk).
pub const NPC_CAPTURES: [(i32, i32, usize, i32, i32); 4] = [
    (3028, 6181, 16, 240, 650),
    (3028, 6180, 16, 240, 650),
    (2063, 5668, 14, 160, 400),
    (2063, 5666, 8, 160, 400),
];

/// Every approved NPC capture frame as a model built from the published pack
/// (`models/npc-<id>.pack.bin`), with its capture name and camera. Exactly 54 entries.
pub fn npc_capture_models() -> Vec<(String, clubscape_renderer::model::Model, i32, i32)> {
    use clubscape_renderer::core::NpcPack;
    let mut out = Vec::new();
    for (npc, sequence, frames, y, z) in NPC_CAPTURES {
        let pack = NpcPack::from_chunks(&read_asset(&format!("models/npc-{npc}.pack.bin")))
            .unwrap_or_else(|e| panic!("published pack for npc {npc}: {e}"));
        for frame in 0..frames {
            let model = pack.frame_model(sequence, frame).unwrap_or_else(|| {
                panic!("published pack for npc {npc} lacks sequence {sequence} frame {frame}")
            });
            out.push((
                format!("npc-{npc}-sequence-{sequence}-frame-{frame}"),
                model,
                y,
                z,
            ));
        }
    }
    assert_eq!(out.len(), 54, "the approved pack has 54 NPC capture frames");
    out
}

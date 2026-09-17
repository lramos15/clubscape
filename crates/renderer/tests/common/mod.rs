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

/// SHA-256 hex digest (workspace `sha2`), used to pin every test input to the manifest.
pub fn sha256_hex(data: &[u8]) -> String {
    use sha2::Digest as _;
    let digest = sha2::Sha256::digest(data);
    digest.iter().map(|b| format!("{b:02x}")).collect()
}

fn manifest() -> serde_json::Value {
    let path = repo_root().join("assets/compiled/render/manifest.json");
    serde_json::from_slice(
        &std::fs::read(&path)
            .unwrap_or_else(|e| panic!("render manifest {} unreadable: {e}", path.display())),
    )
    .expect("render manifest is JSON")
}

/// Pinned SHA-256 of a manifest file entry (`files[key].sha256`).
fn pinned_sha(manifest: &serde_json::Value, key: &str) -> Option<String> {
    manifest["files"][key]["sha256"]
        .as_str()
        .map(str::to_string)
}

/// Reads a render asset by manifest key (`scenes/<name>.bin`, `models/...`, `anim/...`) and
/// returns exactly the pinned bytes: a raw file is used only when its SHA-256 equals the
/// manifest entry (a stale or newer unpublished local export never overrides the published
/// input); otherwise the published gzip twin is inflated and its decompressed hash checked.
/// A key the manifest does not list, or bytes matching neither pin, fail loudly.
pub fn read_asset(key: &str) -> Vec<u8> {
    let manifest = manifest();
    let root = repo_root().join("assets/compiled/render");
    let path = root.join(key);
    let pinned = pinned_sha(&manifest, key);
    if key == "manifest.json" {
        return std::fs::read(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    }
    let twin_key = format!("{key}.gz");
    let twin_pin = manifest["files"][&twin_key]["detail"]["decompressed_sha256"]
        .as_str()
        .map(str::to_string);
    let expected = pinned
        .clone()
        .or_else(|| twin_pin.clone())
        .unwrap_or_else(|| {
            panic!("render input {key} is not listed in assets/compiled/render/manifest.json")
        });
    let mut notes = Vec::new();
    if let Ok(bytes) = std::fs::read(&path) {
        let actual = sha256_hex(&bytes);
        if actual == expected {
            return bytes;
        }
        // Falling back to the pinned published bytes is valid; the stale raw file is only
        // reported (once per read, one line) so the checkout state stays visible in the log.
        let modified = std::fs::metadata(&path)
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| format!("mtime {}s", d.as_secs()))
            .unwrap_or_else(|| "mtime unknown".into());
        eprintln!(
            "stale raw render input ignored: {} ({} bytes, sha256 {}…, {modified}) does not match the manifest pin {}…; using the published gzip twin",
            path.display(),
            bytes.len(),
            &actual[..12],
            &expected[..12]
        );
        notes.push(format!(
            "raw {} has sha256 {actual}, manifest pins {expected} (stale or unpublished local export; ignored)",
            path.display()
        ));
    }
    let twin = root.join(&twin_key);
    match std::fs::read(&twin) {
        Ok(gz) => {
            if let Some(gz_pin) = pinned_sha(&manifest, &twin_key) {
                let gz_actual = sha256_hex(&gz);
                assert!(
                    gz_actual == gz_pin,
                    "published twin {} has sha256 {gz_actual}, manifest pins {gz_pin}",
                    twin.display()
                );
            }
            let bytes = inflate(&gz, key);
            let actual = sha256_hex(&bytes);
            assert!(
                actual == expected,
                "published twin {} inflates to sha256 {actual}, manifest pins {expected}",
                twin.display()
            );
            bytes
        }
        Err(_) => panic!(
            "render input {key} is missing: no raw file with the pinned hash and no published gzip twin {} \
             (the manifest declares it; see tools/render-assets/README.md){}",
            twin.display(),
            if notes.is_empty() {
                String::new()
            } else {
                format!("; {}", notes.join("; "))
            }
        ),
    }
}

/// The manifest's texture buffers (`textures/<id>.bin`), each pinned to its hash; nothing else
/// in the directory is read, so an unpruned local dump cannot leak into a test.
pub fn texture_bytes() -> Vec<Vec<u8>> {
    let manifest = manifest();
    let ids = manifest["textures"].as_array().expect("manifest.textures");
    assert!(!ids.is_empty(), "manifest lists no textures");
    ids.iter()
        .map(|id| {
            read_asset(&format!(
                "textures/{}.bin",
                id.as_i64().expect("texture id")
            ))
        })
        .collect()
}

/// Whether a render asset (raw or gzip twin) exists.
pub fn asset_available(key: &str) -> bool {
    let path = repo_root().join("assets/compiled/render").join(key);
    path.exists() || PathBuf::from(format!("{}.gz", path.display())).exists()
}

/// Compact difference summary of two equally long pixel (or any `i32`) buffers.
#[derive(Debug, Clone, PartialEq)]
pub struct BufferDiff {
    pub len: usize,
    pub differing: usize,
    /// First few differing positions as (index, left, right).
    pub first: Vec<(usize, i32, i32)>,
    pub max_channel: i32,
}

pub fn diff_buffers(a: &[i32], b: &[i32]) -> BufferDiff {
    assert!(
        a.len() == b.len(),
        "buffers differ in length: {} vs {}",
        a.len(),
        b.len()
    );
    let mut diff = BufferDiff {
        len: a.len(),
        differing: 0,
        first: Vec::new(),
        max_channel: 0,
    };
    for (i, (&x, &y)) in a.iter().zip(b).enumerate() {
        if x != y {
            diff.differing += 1;
            if diff.first.len() < 5 {
                diff.first.push((i, x, y));
            }
            for shift in [16, 8, 0] {
                let d = (((x >> shift) & 255) - ((y >> shift) & 255)).abs();
                diff.max_channel = diff.max_channel.max(d);
            }
        }
    }
    diff
}

fn describe(diff: &BufferDiff, width: usize) -> String {
    let positions: Vec<String> = diff
        .first
        .iter()
        .map(|(i, x, y)| {
            if width > 0 {
                format!(
                    "({},{}) {:06x} vs {:06x}",
                    i % width,
                    i / width,
                    x & 0xFF_FFFF,
                    y & 0xFF_FFFF
                )
            } else {
                format!("[{i}] {x} vs {y}")
            }
        })
        .collect();
    format!(
        "{} of {} entries differ, max channel error {}, first: {}",
        diff.differing,
        diff.len,
        diff.max_channel,
        positions.join(", ")
    )
}

/// Strict equality of two pixel buffers with a bounded failure message (count, first
/// differences, max channel error) instead of the whole arrays.
pub fn assert_pixels_equal(actual: &[i32], expected: &[i32], width: usize, what: &str) {
    let diff = diff_buffers(actual, expected);
    assert!(diff.differing == 0, "{what}: {}", describe(&diff, width));
}

/// Two pixel buffers must differ somewhere; the failure message stays bounded.
pub fn assert_pixels_differ(a: &[i32], b: &[i32], what: &str) {
    let diff = diff_buffers(a, b);
    assert!(
        diff.differing > 0,
        "{what}: both buffers are identical ({} entries)",
        diff.len
    );
}

/// Reads a reproducible local export that is deliberately not published (world blocks, pinned
/// validation twins, baked frames). Tests that depend on these are `#[ignore]`d with the
/// reproduction command; when run, a missing file is a failure, and a file the manifest pins
/// must carry exactly the pinned bytes (a stale export fails instead of passing quietly).
pub fn read_local_export(key: &str, reproduce: &str) -> Vec<u8> {
    let path = repo_root().join("assets/compiled/render").join(key);
    let bytes = std::fs::read(&path).unwrap_or_else(|e| {
        panic!(
            "local render export {key} is missing ({e}); reproduce it with `{reproduce}` \
             (this test is ignored by default for that reason)"
        )
    });
    if let Some(pin) = pinned_sha(&manifest(), key) {
        let actual = sha256_hex(&bytes);
        assert!(
            actual == pin,
            "local render export {key} has sha256 {actual}, manifest pins {pin}; \
             reproduce it with `{reproduce}`"
        );
    }
    bytes
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

/// Writes an RGB PNG of packed `0xRRGGBB` pixels to `.local/render-assets/test-output/<name>.png`.
#[allow(dead_code)]
pub fn write_png(name: &str, width: u32, height: u32, pixels: &[i32]) {
    let dir = repo_root().join(".local/render-assets/test-output");
    std::fs::create_dir_all(&dir).unwrap();
    let file = std::fs::File::create(dir.join(format!("{name}.png"))).unwrap();
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), width, height);
    encoder.set_color(png::ColorType::Rgb);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().unwrap();
    let bytes: Vec<u8> = pixels
        .iter()
        .flat_map(|p| [(p >> 16) as u8, (p >> 8) as u8, *p as u8])
        .collect();
    writer.write_image_data(&bytes).unwrap();
}

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

/// Self-contained SHA-256 (FIPS 180-4) so the test inputs can be pinned to the manifest without
/// adding a dependency edge to the workspace lock.
pub fn sha256_hex(data: &[u8]) -> String {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
        0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
        0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
        0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
        0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
        0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
        0xc67178f2,
    ];
    let mut h: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
        0x5be0cd19,
    ];
    let mut message = data.to_vec();
    let bit_len = (data.len() as u64).wrapping_mul(8);
    message.push(0x80);
    while message.len() % 64 != 56 {
        message.push(0);
    }
    message.extend_from_slice(&bit_len.to_be_bytes());
    for block in message.as_chunks::<64>().0 {
        let mut w = [0u32; 64];
        for (i, word) in block.as_chunks::<4>().0.iter().enumerate() {
            w[i] = u32::from_be_bytes(*word);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }
        let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut hh] = h;
        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ (!e & g);
            let t1 = hh
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K[i])
                .wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let t2 = s0.wrapping_add(maj);
            hh = g;
            g = f;
            f = e;
            e = d.wrapping_add(t1);
            d = c;
            c = b;
            b = a;
            a = t1.wrapping_add(t2);
        }
        for (slot, v) in h.iter_mut().zip([a, b, c, d, e, f, g, hh]) {
            *slot = slot.wrapping_add(v);
        }
    }
    h.iter().map(|v| format!("{v:08x}")).collect()
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

//! The published block package index (`blocks.index.json`) must be bound to the published
//! manifest: written for exactly this `manifest.json` and pinning exactly the manifest's
//! block/minimap buffer hashes. Runs on published inputs only (no block exports needed), so a
//! checkout whose index lags a manifest update fails here before `unpack-blocks` installs a
//! package `verify-blocks` would reject.
mod common;

use common::{read_asset, repo_root, sha256_hex};

#[test]
fn block_package_index_is_bound_to_the_published_manifest() {
    let manifest_bytes = read_asset("manifest.json");
    let manifest: serde_json::Value = serde_json::from_slice(&manifest_bytes).unwrap();
    let index_path = repo_root().join("assets/compiled/render/blocks.index.json");
    let index: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&index_path).expect("blocks.index.json published"))
            .unwrap();
    assert_eq!(index["kind"], "clubscape_render_blocks");
    assert_eq!(
        index["manifest_sha256"].as_str().unwrap(),
        sha256_hex(&manifest_bytes),
        "blocks.index.json was written for another manifest.json; run export.py --profile pack-blocks"
    );
    assert_eq!(
        index["approved_reference_pack_sha256"],
        manifest["approved_reference_pack_sha256"]
    );
    // Exactly the manifest's published block twins and minimap sidecars, with its hashes.
    let mut expected: Vec<(String, String)> = Vec::new();
    for block in manifest["blocks"].as_array().unwrap() {
        for key in ["file_gz", "models_file_gz"] {
            let name = block[key].as_str().expect("published gzip twin");
            expected.push((
                name.to_string(),
                manifest["files"][name]["sha256"]
                    .as_str()
                    .unwrap()
                    .to_string(),
            ));
        }
    }
    for entry in manifest["minimap_blocks"].as_array().unwrap() {
        let name = entry["file"].as_str().unwrap();
        expected.push((
            name.to_string(),
            manifest["files"][name]["sha256"]
                .as_str()
                .unwrap()
                .to_string(),
        ));
    }
    expected.sort();
    let mut pinned: Vec<(String, String)> = index["files"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| {
            (
                e["file"].as_str().unwrap().to_string(),
                e["sha256"].as_str().unwrap().to_string(),
            )
        })
        .collect();
    pinned.sort();
    assert_eq!(
        pinned.len(),
        expected.len(),
        "index pins {} files, manifest publishes {}",
        pinned.len(),
        expected.len()
    );
    for (p, e) in pinned.iter().zip(&expected) {
        assert_eq!(p, e, "index entry differs from the manifest pin");
    }
    // The content hash is the deterministic digest of "<file> <sha>\n"-joined entries in
    // manifest order, and names the pack file.
    let names_in_order: Vec<String> = {
        let mut v = Vec::new();
        for block in manifest["blocks"].as_array().unwrap() {
            for key in ["file_gz", "models_file_gz"] {
                v.push(block[key].as_str().unwrap().to_string());
            }
        }
        for entry in manifest["minimap_blocks"].as_array().unwrap() {
            v.push(entry["file"].as_str().unwrap().to_string());
        }
        v
    };
    let joined = names_in_order
        .iter()
        .map(|n| format!("{n} {}", manifest["files"][n]["sha256"].as_str().unwrap()))
        .collect::<Vec<_>>()
        .join("\n");
    let content = sha256_hex(joined.as_bytes());
    assert_eq!(index["content_sha256"].as_str().unwrap(), content);
    assert_eq!(
        index["pack"]["file"].as_str().unwrap(),
        format!("clubscape-render-blocks-{}.tar", &content[..16])
    );
    assert_eq!(
        index["squares"].as_array().unwrap().len(),
        manifest["blocks"].as_array().unwrap().len()
    );
    eprintln!(
        "block package index bound to manifest {}: {} files, content {}, pack {}",
        &sha256_hex(&manifest_bytes)[..12],
        pinned.len(),
        &content[..16],
        index["pack"]["sha256"].as_str().unwrap()
    );
}

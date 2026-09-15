#[cfg(not(target_arch = "wasm32"))]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use std::{fs::File, io::Read, path::Path};

    let input = std::env::args()
        .nth(1)
        .ok_or("Pass the validated world.csc artifact path.")?;
    let path = Path::new(&input);
    let mut bytes = Vec::new();
    File::open(path)?
        .take(clubscape_content::MAX_INPUT_BYTES as u64 + 1)
        .read_to_end(&mut bytes)?;
    let compiled =
        clubscape_content::load_compiled(&bytes, clubscape_content::ValidationMode::Runtime)?;
    if !compiled.report().unresolved_bindings.is_empty() {
        return Err("The source artifact still contains unresolved bindings.".into());
    }
    let source = compiled.definition();
    let regions: std::collections::BTreeMap<_, _> = source
        .regions
        .values()
        .map(|region| {
            (
                region.id.to_string(),
                serde_json::json!({"name":region.name,"sceneAsset":region.scene_asset}),
            )
        })
        .collect();
    println!(
        "{}",
        serde_json::json!({
            "schemaVersion":1,
            "artifactSha256":clubscape_content::sha256(&bytes),
            "catalog":clubscape_wasm::catalog::from_compiled(&compiled),
            "regions":regions,
            "referencedAssets":compiled.referenced_assets(),
        })
    );
    Ok(())
}

#[cfg(target_arch = "wasm32")]
fn main() {}

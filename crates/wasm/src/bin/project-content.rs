#[cfg(not(target_arch = "wasm32"))]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use std::{fs::File, io::Read, path::Path};

    let input = std::env::args()
        .nth(1)
        .ok_or("Pass the validated world.csc artifact path.")?;
    let path = Path::new(&input);
    let mut bytes = Vec::new();
    let source: Box<dyn Read> = if input == "-" {
        Box::new(std::io::stdin())
    } else {
        Box::new(File::open(path)?)
    };
    source
        .take(clubscape_content::MAX_INPUT_BYTES as u64 + 1)
        .read_to_end(&mut bytes)?;
    let compiled =
        clubscape_content::load_compiled(&bytes, clubscape_content::ValidationMode::Runtime)?;
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
    let instance_layouts: std::collections::BTreeMap<_, _> = source
        .mechanics
        .instances
        .values()
        .map(|template| {
            let chunks: Vec<_> = template
                .chunks
                .iter()
                .map(|chunk| {
                    serde_json::json!({
                        "sourceRegion":chunk.source_region,
                        "sourceOrigin":chunk.source_origin,
                        "destinationRegion":chunk.destination_region,
                        "destinationOrigin":chunk.destination_origin,
                        "quarterTurns":chunk.quarter_turns,
                    })
                })
                .collect();
            (
                template.id.to_string(),
                serde_json::json!({"chunkSize":template.chunk_size,"chunks":chunks}),
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
            "instanceLayouts":instance_layouts,
            "referencedAssets":compiled.referenced_assets(),
            "contentValidation":{
                "contentSchemaVersion":source.schema_version,
                "artifactVersion":clubscape_content::ARTIFACT_VERSION,
                "unresolvedBindings":compiled.report().unresolved_bindings,
                "readinessAuthority":"server_readiness_profile",
                "runtimeReadinessEstablished":false,
            },
        })
    );
    Ok(())
}

#[cfg(target_arch = "wasm32")]
fn main() {}

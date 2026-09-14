use std::{env, fs};

use clubscape_game_types::GameContent;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = env::args()
        .nth(1)
        .ok_or("expected a GameContent JSON path")?;
    let data = fs::read(path)?;
    let content: GameContent = serde_json::from_slice(&data)?;
    println!(
        "{}",
        serde_json::json!({
            "canonical_game_content_deserialized": true,
            "runtime_compiled": false,
            "gameplay_executed": false,
            "revision": content.revision,
            "items": content.items.len(),
            "skills": content.skills.len(),
            "regions": content.regions.len(),
            "spawns": content.spawns.len(),
            "interfaces": content.interfaces.len(),
            "recipes": content.recipes.len(),
        })
    );
    Ok(())
}

//! Strict, headless validation and deterministic compilation of shared game content.
//!
//! Compilation checks internal contracts, not source truth, gameplay completeness,
//! owner approval, or presentation fidelity. See the crate README for the exact
//! rules, resource limits, and the versioned artifact format.

mod artifact;
mod decode;
mod validation;

use std::collections::{BTreeMap, BTreeSet};

use clubscape_game_types::*;
use serde::Serialize;

pub use artifact::{
    ARTIFACT_HEADER_BYTES, ARTIFACT_VERSION, encode_compiled, load_compiled, sha256,
};
pub use decode::{MAX_INPUT_BYTES, read_content_json};

/// Runtime and development share the strict provenance policy.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ValidationMode {
    #[default]
    Runtime,
    TestFixture,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct EvidenceCounts {
    pub verified_reference: usize,
    pub inference: usize,
    pub approved_adaptation: usize,
    pub test_fixture: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ContentCounts {
    pub items: usize,
    pub skills: usize,
    pub regions: usize,
    pub collision_cells: usize,
    pub spawns: usize,
    pub objects: usize,
    pub npcs: usize,
    pub recipes: usize,
    pub dialogues: usize,
    pub dialogue_nodes: usize,
    pub tutorial_stages: usize,
    pub quests: usize,
    pub shops: usize,
    pub interfaces: usize,
    pub equipment_slots: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ValidationReport {
    pub mode: ValidationMode,
    pub checks: Vec<&'static str>,
    /// Counts record occurrences, not independently verified facts.
    pub evidence: EvidenceCounts,
    /// Exact typed definition paths; compilation does not replace these with usable defaults.
    pub unresolved_bindings: Vec<String>,
    pub referenced_assets: usize,
    pub unassigned_asset_sites: usize,
    pub asset_manifest: Option<String>,
    pub interface_definition_validation_performed: bool,
    pub recipe_tool_reference_validation_performed: bool,
    pub source_verification_performed: bool,
    pub approval_verification_performed: bool,
    pub presentation_verification_performed: bool,
    pub limitations: Vec<&'static str>,
}

/// A caller-supplied manifest projection. This does not assert file availability.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AssetManifest {
    pub identity: String,
    pub assets: BTreeSet<AssetId>,
}

#[derive(Clone, Debug)]
pub(crate) struct CellIndex {
    region: RegionId,
    offset: usize,
}

/// Validated definitions with rebuilt, ordered indexes. No mutable access is
/// exposed: mutation requires compiling a new definition.
#[derive(Clone, Debug)]
pub struct CompiledContent {
    definition: GameContent,
    collision: BTreeMap<Tile, CellIndex>,
    spawns_at: BTreeMap<Tile, Vec<SpawnId>>,
    dialogue_nodes: BTreeMap<DialogueId, BTreeMap<String, usize>>,
    assets: BTreeSet<AssetId>,
    report: ValidationReport,
}

pub fn compile_content(
    definition: GameContent,
    mode: ValidationMode,
) -> GameResult<CompiledContent> {
    let validated = match validation::validate(&definition, mode) {
        Ok(validated) => validated,
        Err(error) => {
            validation::discard_rules(definition);
            return Err(error);
        }
    };
    Ok(CompiledContent {
        definition,
        collision: validated.collision,
        spawns_at: validated.spawns_at,
        dialogue_nodes: validated.dialogue_nodes,
        assets: validated.assets,
        report: validated.report,
    })
}

pub fn compile_content_with_manifest(
    definition: GameContent,
    mode: ValidationMode,
    manifest: &AssetManifest,
) -> GameResult<CompiledContent> {
    let mut compiled = compile_content(definition, mode)?;
    compiled.check_asset_manifest(manifest)?;
    Ok(compiled)
}

impl CompiledContent {
    pub fn definition(&self) -> &GameContent {
        &self.definition
    }

    pub fn report(&self) -> &ValidationReport {
        &self.report
    }

    pub fn counts(&self) -> ContentCounts {
        let content = &self.definition;
        ContentCounts {
            items: content.items.len(),
            skills: content.skills.len(),
            regions: content.regions.len(),
            collision_cells: self.collision.len(),
            spawns: content.spawns.len(),
            objects: content.objects.len(),
            npcs: content.npcs.len(),
            recipes: content.recipes.len(),
            dialogues: content.dialogues.len(),
            dialogue_nodes: self.dialogue_nodes.values().map(BTreeMap::len).sum(),
            tutorial_stages: content.tutorial.len(),
            quests: content.quests.len(),
            shops: content.shops.len(),
            interfaces: content.interfaces.len(),
            equipment_slots: content.equipment_slots.len(),
        }
    }

    /// Missing cells have no collision definition; they are not walkable defaults.
    pub fn collision(&self, tile: Tile) -> Option<&CollisionCell> {
        let index = self.collision.get(&tile)?;
        self.definition
            .regions
            .get(&index.region)?
            .cells
            .get(index.offset)
    }

    /// Ownership follows explicit cells, not overlapping region bounding boxes.
    pub fn region_at(&self, tile: Tile) -> Option<&RegionDefinition> {
        self.region(&self.collision.get(&tile)?.region)
    }

    pub fn spawns_at_tile(&self, tile: Tile) -> impl Iterator<Item = &SpawnDefinition> {
        self.spawns_at
            .get(&tile)
            .into_iter()
            .flatten()
            .filter_map(|id| self.definition.spawns.get(id))
    }

    pub fn item(&self, id: &ItemId) -> Option<&ItemDefinition> {
        self.definition.items.get(id)
    }

    pub fn skill(&self, id: &SkillId) -> Option<&SkillDefinition> {
        self.definition.skills.get(id)
    }

    pub fn region(&self, id: &RegionId) -> Option<&RegionDefinition> {
        self.definition.regions.get(id)
    }

    pub fn spawn(&self, id: &SpawnId) -> Option<&SpawnDefinition> {
        self.definition.spawns.get(id)
    }

    pub fn object(&self, id: &ObjectId) -> Option<&ObjectDefinition> {
        self.definition.objects.get(id)
    }

    pub fn npc(&self, id: &NpcId) -> Option<&NpcDefinition> {
        self.definition.npcs.get(id)
    }

    pub fn recipe(&self, id: &RecipeId) -> Option<&RecipeDefinition> {
        self.definition.recipes.get(id)
    }

    pub fn dialogue(&self, id: &DialogueId) -> Option<&DialogueDefinition> {
        self.definition.dialogues.get(id)
    }

    pub fn dialogue_node(&self, dialogue: &DialogueId, node: &str) -> Option<&DialogueNode> {
        let offset = self.dialogue_nodes.get(dialogue)?.get(node)?;
        self.dialogue(dialogue)?.nodes.get(*offset)
    }

    pub fn tutorial_stage(&self, id: &StageId) -> Option<&TutorialStageDefinition> {
        self.definition.tutorial.get(id)
    }

    pub fn quest(&self, id: &QuestId) -> Option<&QuestDefinition> {
        self.definition.quests.get(id)
    }

    pub fn shop(&self, id: &ShopId) -> Option<&ShopDefinition> {
        self.definition.shops.get(id)
    }

    pub fn interface(&self, id: &InterfaceId) -> Option<&InterfaceDefinition> {
        self.definition.interfaces.get(id)
    }

    pub fn has_equipment_slot(&self, id: &SlotId) -> bool {
        self.definition.equipment_slots.contains(id)
    }

    pub fn referenced_assets(&self) -> &BTreeSet<AssetId> {
        &self.assets
    }

    /// Check ID membership only. Geometry, files, hashes, audio, and presentation
    /// need their own asset-build/acceptance validation.
    pub fn check_asset_manifest(&mut self, manifest: &AssetManifest) -> GameResult<()> {
        validation::identity(&manifest.identity, "asset_manifest.identity")?;
        for asset in &self.assets {
            if !manifest.assets.contains(asset) {
                return Err(invalid(
                    "asset_manifest",
                    format!(
                        "referenced asset {asset} is absent from {}",
                        manifest.identity
                    ),
                ));
            }
        }
        self.report.asset_manifest = Some(manifest.identity.clone());
        if !self.report.checks.contains(&"asset_manifest_membership") {
            self.report.checks.push("asset_manifest_membership");
        }
        Ok(())
    }
}

pub(crate) fn invalid(path: &str, message: impl std::fmt::Display) -> GameError {
    GameError::new(GameErrorCode::InvalidContent, format!("{path}: {message}"))
}

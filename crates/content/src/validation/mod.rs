mod definitions;
mod execution;
mod graphs;
mod mechanics;
mod rules;
mod world;

pub(crate) use rules::discard_rules;

use std::collections::{BTreeMap, BTreeSet};

use clubscape_game_types::*;

use crate::{
    CellIndex, EvidenceCounts, ValidationMode, ValidationReport,
    decode::{MAX_COLLECTION_ENTRIES, MAX_TEXT_BYTES},
    invalid,
};

pub(crate) const MAX_RULE_DEPTH: usize = 32;
pub(crate) const MAX_RULE_NODES: usize = 100_000;
pub(crate) const MAX_TOOL_PLACEMENT_STEPS: usize = 100_000;

pub(crate) struct Validated {
    pub collision: BTreeMap<Tile, CellIndex>,
    pub spawns_at: BTreeMap<Tile, Vec<SpawnId>>,
    pub dialogue_nodes: BTreeMap<DialogueId, BTreeMap<String, usize>>,
    pub assets: BTreeSet<AssetId>,
    pub report: ValidationReport,
}

struct Validator<'a> {
    content: &'a GameContent,
    slots: BTreeSet<SlotId>,
    mutable_flags: BTreeSet<String>,
    collision: BTreeMap<Tile, CellIndex>,
}

pub(crate) fn validate(content: &GameContent, mode: ValidationMode) -> GameResult<Validated> {
    if content.schema_version != CONTENT_SCHEMA_VERSION {
        return Err(invalid(
            "schema_version",
            "only GameContent schema version 3 is supported; explicitly migrate and recompile older definitions",
        ));
    }
    identity(&content.revision, "revision")?;
    identity(&content.baseline, "baseline")?;
    if mode == ValidationMode::Runtime && fixture_reference(&content.baseline) {
        return Err(invalid(
            "baseline",
            "a test-fixture identity is not runtime source content",
        ));
    }
    macro_rules! check_maps {
        ($($field:ident),+ $(,)?) => {$(
            nonempty(content.$field.len(), stringify!($field))?;
            for (key, definition) in &content.$field {
                if key != &definition.id {
                    return Err(invalid(
                        &format!("{}.{key}.id", stringify!($field)),
                        format!("map key differs from contained ID {}", definition.id),
                    ));
                }
            }
        )+};
    }
    check_maps!(
        items, skills, regions, spawns, objects, npcs, recipes, dialogues, tutorial, quests, shops,
        interfaces,
    );
    nonempty(content.equipment_slots.len(), "equipment_slots")?;
    let slots = unique(content.equipment_slots.iter(), "equipment_slots")?
        .into_iter()
        .cloned()
        .collect();
    let scan = rules::scan(content)?;
    let mut validator = Validator {
        content,
        slots,
        mutable_flags: scan.mutable_flags,
        collision: BTreeMap::new(),
    };
    let (evidence, unresolved_bindings) = validator.sources(mode)?;
    validator.skills()?;
    validator.items()?;
    validator.interface_definitions()?;
    validator.regions()?;
    validator.mechanics()?;
    validator.objects_and_npcs()?;
    validator.recipes()?;
    validator.shops()?;
    let spawns_at = validator.spawns()?;
    let dialogue_nodes = validator.dialogues()?;
    validator.progression_definitions()?;
    validator.initial_state()?;
    validator.progression_graphs()?;
    let (assets, unassigned_asset_sites) = validator.assets();
    let report = ValidationReport {
        mode,
        checks: vec![
            "schema_and_identity",
            "map_keys_and_source_ids",
            "source_record_structure_and_mode",
            "numeric_and_reference_contracts",
            "explicit_collision_ownership",
            "initial_state_and_containers",
            "bounded_guard_and_effect_trees",
            "dialogue_and_progression_graphs",
            "rooted_progression_event_flag_and_unlock_dependencies",
            "one_time_quest_rewards",
            "asset_id_structure",
            "interface_registry_references",
            "recipe_tool_references_and_nonconsumption",
            "recipe_tool_holding_capacity",
            "initial_run_energy_units_and_bounds",
            "typed_mechanic_bindings_and_source_domains",
            "counter_entitlement_instance_and_recovery_contracts",
            "stationary_anchor_and_mobile_footprint_policy",
        ],
        evidence,
        unresolved_bindings,
        referenced_assets: assets.len(),
        unassigned_asset_sites,
        asset_manifest: None,
        interface_definition_validation_performed: true,
        recipe_tool_reference_validation_performed: true,
        source_verification_performed: false,
        approval_verification_performed: false,
        presentation_verification_performed: false,
        limitations: vec![
            "Interface registry validation establishes logical identity and source mappings, not rendered controls or interface behavior.",
            "Source URLs, observations, baseline completeness, and owner approvals are not authenticated by compilation.",
            "Graph checks are conservative structural/constant-condition checks, not a gameplay planner or milestone acceptance.",
            "Typed formula/timing bindings are validated, not executed or source-certified; unresolved bindings remain unavailable at execution.",
            "Animation actor filters are checked as ActorId syntax; dynamic actor existence belongs to world-state validation.",
            "Optional presentation assets, asset files, geometry, audio, and visual fidelity require separate acceptance checks.",
        ],
    };
    Ok(Validated {
        collision: validator.collision,
        spawns_at,
        dialogue_nodes,
        assets,
        report,
    })
}

pub(crate) fn identity(value: &str, path: &str) -> GameResult<()> {
    text(value, path, 256)?;
    if value.chars().any(char::is_control) {
        return Err(invalid(path, "identity must be a single line"));
    }
    if matches!(
        value.to_ascii_lowercase().as_str(),
        "unknown" | "none" | "null" | "todo" | "tbd" | "latest" | "placeholder" | "unset"
    ) {
        return Err(invalid(
            path,
            "requires an identifiable revision/reference, not a placeholder",
        ));
    }
    Ok(())
}

fn text(value: &str, path: &str, maximum: usize) -> GameResult<()> {
    if value.trim().is_empty()
        || value.trim() != value
        || value.len() > maximum
        || value
            .chars()
            .any(|character| character.is_control() && character != '\n')
    {
        return Err(invalid(
            path,
            format!("requires nonblank, trimmed text of at most {maximum} bytes"),
        ));
    }
    Ok(())
}

fn token(value: &str, path: &str) -> GameResult<()> {
    text(value, path, 160)?;
    if !value
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || b"_.:-/".contains(&byte))
    {
        return Err(invalid(path, "requires an identifier without whitespace"));
    }
    Ok(())
}

fn bounded(length: usize, path: &str) -> GameResult<()> {
    if length > MAX_COLLECTION_ENTRIES {
        return Err(invalid(path, "collection length limit exceeded"));
    }
    Ok(())
}

fn nonempty(length: usize, path: &str) -> GameResult<()> {
    bounded(length, path)?;
    if length == 0 {
        return Err(invalid(
            path,
            "missing mandatory content; collection must not be empty",
        ));
    }
    Ok(())
}

fn unique<'a, T: Ord + std::fmt::Display>(
    values: impl IntoIterator<Item = &'a T>,
    path: &str,
) -> GameResult<BTreeSet<&'a T>> {
    let mut result = BTreeSet::new();
    for value in values {
        if !result.insert(value) {
            return Err(invalid(path, format!("duplicate {value}")));
        }
        bounded(result.len(), path)?;
    }
    Ok(result)
}

fn fixture_reference(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    lower.starts_with("fixture:") || lower.starts_with("test-fixture:")
}

impl Validator<'_> {
    fn sources(&self, mode: ValidationMode) -> GameResult<(EvidenceCounts, Vec<String>)> {
        let mut counts = EvidenceCounts::default();
        macro_rules! sources {
            ($($field:ident),+ $(,)?) => {$(
                for (id, definition) in &self.content.$field {
                    source_records(
                        &definition.source,
                        &format!("{}.{id}.source", stringify!($field)),
                        mode,
                        &mut counts,
                    )?;
                }
            )+};
        }
        sources!(
            items, skills, regions, spawns, objects, npcs, recipes, dialogues, tutorial, quests,
            shops, interfaces,
        );
        source_records(
            &self.content.initial_state.source,
            "initial_state.source",
            mode,
            &mut counts,
        )?;
        let mut unresolved = Vec::new();
        self.mechanic_sources(mode, &mut counts, &mut unresolved)?;
        unresolved.sort();
        Ok((counts, unresolved))
    }

    fn assets(&self) -> (BTreeSet<AssetId>, usize) {
        let mut assets = BTreeSet::new();
        let mut unassigned = 0;
        let mut add = |asset: &Option<AssetId>| {
            if let Some(asset) = asset {
                assets.insert(asset.clone());
            } else {
                unassigned += 1;
            }
        };
        for definition in self.content.items.values() {
            add(&definition.asset);
        }
        for definition in self.content.npcs.values() {
            add(&definition.asset);
        }
        for definition in self.content.objects.values() {
            add(&definition.asset);
        }
        for definition in self.content.regions.values() {
            add(&definition.scene_asset);
        }
        for spawn in self.content.spawns.values() {
            for interaction in &spawn.interactions {
                if let InteractionAction::Gather { rule } = &interaction.action {
                    add(&rule.animation);
                    add(&rule.sound);
                }
            }
        }
        for transition in self
            .content
            .tutorial
            .values()
            .flat_map(|stage| &stage.transitions)
            .chain(
                self.content
                    .quests
                    .values()
                    .flat_map(|quest| &quest.transitions),
            )
        {
            if transition.event == "sound"
                && let Some(target) = &transition.target
            {
                assets.insert(AssetId::new(target).expect("validated sound event asset"));
            }
            for projectile in self.content.mechanics.projectiles.values() {
                if let Some(asset) = &projectile.asset {
                    assets.insert(asset.clone());
                } else {
                    unassigned += 1;
                }
            }
        }
        (assets, unassigned)
    }

    fn item(&self, id: &ItemId, path: &str) -> GameResult<&ItemDefinition> {
        self.content
            .items
            .get(id)
            .ok_or_else(|| invalid(path, format!("undefined item {id}")))
    }

    fn skill(&self, id: &SkillId, path: &str) -> GameResult<&SkillDefinition> {
        self.content
            .skills
            .get(id)
            .ok_or_else(|| invalid(path, format!("undefined skill {id}")))
    }

    fn quest_stage(&self, quest: &QuestId, stage: &StageId, path: &str) -> GameResult<()> {
        let definition = self
            .content
            .quests
            .get(quest)
            .ok_or_else(|| invalid(path, format!("undefined quest {quest}")))?;
        if !definition.journal.contains_key(stage) {
            return Err(invalid(
                path,
                format!("undefined stage {stage} in quest {quest}"),
            ));
        }
        Ok(())
    }

    fn tutorial_stage(&self, stage: &StageId, path: &str) -> GameResult<()> {
        if !self.content.tutorial.contains_key(stage) {
            return Err(invalid(path, format!("undefined tutorial stage {stage}")));
        }
        Ok(())
    }

    fn interface(&self, interface: &InterfaceId, path: &str) -> GameResult<()> {
        if !self.content.interfaces.contains_key(interface) {
            return Err(invalid(
                path,
                format!("undefined interface {interface} in the logical interface registry"),
            ));
        }
        Ok(())
    }

    fn requirement(&self, requirement: &SkillRequirement, path: &str) -> GameResult<()> {
        let skill = self.skill(&requirement.skill, path)?;
        if requirement.level == 0
            || usize::from(requirement.level) > skill.xp_thresholds_tenths.len()
        {
            return Err(invalid(
                path,
                format!("invalid level {} for {}", requirement.level, skill.id),
            ));
        }
        Ok(())
    }

    fn requirements(&self, requirements: &[SkillRequirement], path: &str) -> GameResult<()> {
        bounded(requirements.len(), path)?;
        unique(
            requirements.iter().map(|requirement| &requirement.skill),
            path,
        )?;
        for requirement in requirements {
            self.requirement(requirement, path)?;
        }
        Ok(())
    }

    fn xp(&self, rewards: &[XpReward], path: &str) -> GameResult<()> {
        bounded(rewards.len(), path)?;
        unique(rewards.iter().map(|reward| &reward.skill), path)?;
        for reward in rewards {
            let skill = self.skill(&reward.skill, path)?;
            if reward.amount_tenths == 0 || reward.amount_tenths > skill.maximum_xp_tenths {
                return Err(invalid(
                    path,
                    format!(
                        "XP award for {} must be positive and within its maximum XP",
                        skill.id
                    ),
                ));
            }
        }
        Ok(())
    }

    fn stacks(&self, stacks: &[ItemStack], path: &str, inventory_bound: bool) -> GameResult<()> {
        bounded(stacks.len(), path)?;
        unique(stacks.iter().map(|stack| &stack.item), path)?;
        let mut slots = 0_u64;
        for stack in stacks {
            let item = self.item(&stack.item, path)?;
            if stack.quantity.get() == 0 || stack.quantity.get() > MAX_STACK_QUANTITY {
                return Err(invalid(path, "item quantity is outside the stack limit"));
            }
            self.item_instance(stack, path)?;
            slots += if item.stackable.is_always() {
                1
            } else {
                u64::from(stack.quantity.get())
            };
        }
        if inventory_bound && slots > INVENTORY_SLOTS as u64 {
            return Err(invalid(
                path,
                "item batch cannot fit in an ordinary 28-slot inventory",
            ));
        }
        Ok(())
    }
}

fn source_records(
    records: &[SourceRecord],
    path: &str,
    mode: ValidationMode,
    counts: &mut EvidenceCounts,
) -> GameResult<()> {
    nonempty(records.len(), path)?;
    let mut seen = BTreeSet::new();
    for (index, record) in records.iter().enumerate() {
        let path = format!("{path}[{index}]");
        text(&record.reference, &format!("{path}.reference"), 4096)?;
        if record.reference.chars().any(char::is_control) {
            return Err(invalid(&path, "source reference must be a single locator"));
        }
        identity(&record.revision, &format!("{path}.revision"))?;
        text(&record.notes, &format!("{path}.notes"), MAX_TEXT_BYTES)?;
        if !record.reference.contains(['/', ':', '.', '#']) {
            return Err(invalid(
                &path,
                "source reference must identify a URL, file, or fixture",
            ));
        }
        if !record
            .reference
            .bytes()
            .any(|byte| byte.is_ascii_alphanumeric())
        {
            return Err(invalid(
                &path,
                "source reference has no identifiable resource",
            ));
        }
        if let Some((scheme, resource)) = record.reference.split_once("://")
            && (resource.is_empty()
                || (matches!(scheme, "http" | "https")
                    && (resource.starts_with('/') || resource.chars().any(char::is_whitespace))))
        {
            return Err(invalid(&path, "source URL has no valid resource/authority"));
        }
        if fixture_reference(&record.reference)
            && record
                .reference
                .split_once(':')
                .is_none_or(|(_, resource)| {
                    !resource.bytes().any(|byte| byte.is_ascii_alphanumeric())
                })
        {
            return Err(invalid(
                &path,
                "fixture reference must identify its synthetic definition",
            ));
        }
        if !seen.insert((&record.reference, &record.revision)) {
            return Err(invalid(
                &path,
                "duplicate source reference/revision on one definition",
            ));
        }
        if mode == ValidationMode::Runtime
            && (record.status == EvidenceStatus::TestFixture
                || fixture_reference(&record.reference))
        {
            return Err(invalid(
                &path,
                "TestFixture provenance is forbidden in runtime/development mode",
            ));
        }
        match record.status {
            EvidenceStatus::VerifiedReference => counts.verified_reference += 1,
            EvidenceStatus::Inference => counts.inference += 1,
            EvidenceStatus::ApprovedAdaptation => counts.approved_adaptation += 1,
            EvidenceStatus::TestFixture => {
                if !fixture_reference(&record.reference) {
                    return Err(invalid(
                        &path,
                        "TestFixture records require an explicit fixture: reference",
                    ));
                }
                counts.test_fixture += 1;
            }
        }
    }
    Ok(())
}

fn chance(rule: &ChanceRule, path: &str, require_possible: bool) -> GameResult<()> {
    rule.validate().map_err(|error| invalid(path, error))?;
    let (minimum, maximum) = match rule.domain {
        ChanceDomain::Constant => (1, 1),
        ChanceDomain::Skill { levels } => (levels.minimum, levels.maximum),
    };
    if require_possible
        && rule
            .numerator(minimum)
            .map_err(|error| invalid(path, error))?
            == 0
        && rule
            .numerator(maximum)
            .map_err(|error| invalid(path, error))?
            == 0
    {
        return Err(invalid(
            path,
            "successful action is impossible at every level",
        ));
    }
    Ok(())
}

fn bonuses(bonuses: &CombatBonuses, path: &str) -> GameResult<()> {
    bounded(bonuses.attack.len(), path)?;
    bounded(bonuses.defence.len(), path)?;
    Ok(())
}

fn source_id<'a, T: Ord + Copy>(
    seen: &mut BTreeMap<T, &'a str>,
    number: T,
    id: &'a str,
    path: &str,
) -> GameResult<()> {
    if let Some(previous) = seen.insert(number, id) {
        return Err(invalid(
            path,
            format!("duplicate category-local source ID shared by {previous} and {id}"),
        ));
    }
    Ok(())
}

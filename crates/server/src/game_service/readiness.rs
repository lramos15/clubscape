use std::collections::{BTreeMap, BTreeSet};

use clubscape_content::CompiledContent;
use clubscape_game_types::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Profile {
    pub id: String,
    pub excluded_items: BTreeSet<ItemId>,
}

#[derive(Clone, Debug, Default, Serialize)]
pub(crate) struct Readiness {
    pub profile: Option<String>,
    pub inactive: BTreeMap<String, String>,
    #[serde(skip)]
    excluded: BTreeSet<ItemId>,
    #[serde(skip)]
    ordinary_office: bool,
    #[serde(skip)]
    possible_office_items: BTreeSet<ItemId>,
}

impl Readiness {
    pub fn check(compiled: &CompiledContent, profile: Option<&Profile>) -> GameResult<Self> {
        let content = compiled.definition();
        let mut result = Self::default();
        let Some(profile) = profile else {
            if compiled.report().unresolved_bindings.is_empty() {
                return Ok(result);
            }
            return Err(invalid("Required source bindings remain unresolved."));
        };
        if profile.id != "ordinary_normal_f2p" || content.mechanics.world_members != Some(false) {
            return Err(invalid(
                "Readiness profile requires the declared normal F2P world.",
            ));
        }
        let (possible, instances) = possible_items(content)?;
        for item in &profile.excluded_items {
            if !content.items.contains_key(item) || possible.contains(item) {
                return Err(invalid(
                    "An excluded alternative has an available item-introduction path.",
                ));
            }
        }
        result.profile = Some(profile.id.clone());
        result.excluded = profile.excluded_items.clone();
        let mut proofs = BTreeMap::new();
        for item in &profile.excluded_items {
            if let Stackability::Conditional {
                rule: SourceBinding::Unresolved { .. },
                ..
            } = &content.items[item].stackable
            {
                proofs.insert(format!("items.{item}.stackable.rule"),
                    "No initial, ground, shop, grant, reconciliation, recipe, transformation or non-members loot introduction.".to_owned());
            }
        }
        for (spawn, definition) in &content.spawns {
            for (index, interaction) in definition.interactions.iter().enumerate() {
                if let InteractionAction::Gather { rule } = &interaction.action
                    && let Some(mechanics) = &rule.mechanics
                    && matches!(mechanics.respawn, SourceBinding::Unresolved { .. })
                    && matches!(rule.depletion.domain, ChanceDomain::Constant)
                    && rule.depletion.numerator_at_level_1 == 0
                    && rule.depletion.numerator_at_level_99 == 0
                {
                    proofs.insert(format!("spawns.{spawn}.interactions[{index}].action.rule.mechanics.respawn"),
                        "Source depletion is exactly zero; relocation is independently validated, not waived.".to_owned());
                }
            }
        }
        if let Some(death) = &content.mechanics.death
            && matches!(death.office_overflow, SourceBinding::Unresolved { .. })
            && possible.len() <= usize::from(death.office_capacity)
            && !instances
            && possible.iter().all(|id| {
                content.items.get(id).is_some_and(|item| {
                    item.charges.is_none() && matches!(item.stackable, Stackability::Simple(_))
                })
            })
        {
            result.ordinary_office = true;
            result.possible_office_items = possible.clone();
            proofs.insert("mechanics.death.office_overflow".to_owned(),
                format!("At most {} ordinary item keys, no introduced item instances, within {} Office slots.",
                    possible.len(), death.office_capacity));
        }
        for path in &compiled.report().unresolved_bindings {
            let proof = proofs.get(path).ok_or_else(|| {
                invalid("A required source binding lacks an executable inactivity proof.")
            })?;
            result.inactive.insert(path.clone(), proof.clone());
        }
        Ok(result)
    }

    /// A retained/restarted world cannot bypass the profile's acquisition/instance proof.
    pub fn validate_world(&self, world: &WorldState) -> GameResult<()> {
        let check = |stack: &ItemStack| {
            !self.excluded.contains(&stack.item)
                && (!self.ordinary_office
                    || (stack.instance.is_none()
                        && self.possible_office_items.contains(&stack.item)))
        };
        for character in world.characters.values() {
            if !character
                .inventory
                .slots
                .iter()
                .flatten()
                .chain(character.equipment.values())
                .chain(character.bank.slots.iter().flatten())
                .all(check)
            {
                return Err(invalid(
                    "Persisted items violate the selected readiness profile.",
                ));
            }
        }
        if world.ground_items.iter().any(|item| !check(&item.stack))
            || world
                .runtime
                .projectiles
                .iter()
                .flat_map(|item| &item.resources_spent)
                .any(|item| !check(item))
            || world.shops.values().any(|shop| {
                shop.stock.iter().any(|(item, count)| {
                    *count > 0
                        && (self.excluded.contains(item)
                            || (self.ordinary_office && !self.possible_office_items.contains(item)))
                })
            })
            || world.runtime.deaths.values().any(|death| {
                death
                    .retained
                    .iter()
                    .chain(death.office.iter())
                    .chain(death.grave.iter().flat_map(|grave| &grave.items))
                    .any(|item| !check(&item.stack))
            })
        {
            return Err(invalid(
                "World/recovery items violate the selected readiness profile.",
            ));
        }
        if self.profile.is_some() && world.runtime.members != Some(false) {
            return Err(invalid(
                "The world no longer matches the readiness profile.",
            ));
        }
        Ok(())
    }
}

fn invalid(message: &'static str) -> GameError {
    GameError::new(GameErrorCode::InvalidContent, message)
}

fn possible_items(content: &GameContent) -> GameResult<(BTreeSet<ItemId>, bool)> {
    let mut items = BTreeSet::new();
    let mut instances = false;
    let mut stacks = Vec::new();
    stacks.extend(content.initial_state.inventory.slots.iter().flatten());
    stacks.extend(content.initial_state.equipment.values());
    stacks.extend(content.initial_state.bank.slots.iter().flatten());
    let mut effects = Vec::new();
    for spawn in content.spawns.values() {
        if let SpawnKind::Item { stack, .. } = &spawn.kind {
            stacks.push(stack);
        }
        for interaction in &spawn.interactions {
            collect_interaction(&interaction.action, &mut stacks, &mut effects);
        }
    }
    for recipe in content.recipes.values() {
        stacks.extend(&recipe.outputs);
        stacks.extend(&recipe.failed_outputs);
        if let Some(mechanics) = &recipe.mechanics {
            effects.extend(&mechanics.success_effects);
            effects.extend(&mechanics.failure_effects);
        }
    }
    for transition in content
        .tutorial
        .values()
        .flat_map(|stage| &stage.transitions)
        .chain(content.quests.values().flat_map(|quest| &quest.transitions))
    {
        effects.extend(&transition.effects);
    }
    for choice in content
        .dialogues
        .values()
        .flat_map(|dialogue| &dialogue.nodes)
        .flat_map(|node| &node.choices)
    {
        effects.extend(&choice.effects);
    }
    for travel in content.mechanics.travels.values() {
        effects.extend(&travel.completion_effects);
    }
    for object in content.mechanics.temporary_objects.values() {
        stacks.extend(&object.expired_items);
        for interaction in &object.interactions {
            collect_interaction(&interaction.action, &mut stacks, &mut effects);
        }
    }
    for grant in content.mechanics.grants.values() {
        items.extend(grant.lines.iter().map(|line| line.item.clone()));
    }
    for reconciliation in content.mechanics.reconciliations.values() {
        if let SourceBinding::Bound { value, .. } = &reconciliation.policies {
            for policy in value {
                match policy {
                    ContainerReconciliation::ReplaceInventory { inventory } => {
                        stacks.extend(inventory.slots.iter().flatten())
                    }
                    ContainerReconciliation::ReplaceEquipment { equipment } => {
                        stacks.extend(equipment.values())
                    }
                    ContainerReconciliation::ReplaceBank { slots } => {
                        stacks.extend(slots.iter().flatten())
                    }
                    _ => {}
                }
            }
        }
    }
    while let Some(effect) = effects.pop() {
        match effect {
            Effect::GiveItems { items } => stacks.extend(items),
            Effect::Conditional {
                effects: nested, ..
            }
            | Effect::Once {
                effects: nested, ..
            } => effects.extend(nested),
            _ => {}
        }
    }
    for stack in stacks {
        items.insert(stack.item.clone());
        instances |= stack.instance.is_some();
    }
    for shop in content.shops.values() {
        items.extend(
            shop.stock
                .iter()
                .filter(|line| line.base_stock > 0)
                .map(|line| line.item.clone()),
        );
    }
    let mut pools = Vec::new();
    for npc in content.npcs.values() {
        if let Some(combat) = &npc.combat {
            items.extend(
                combat
                    .drops
                    .iter()
                    .filter(|drop| drop.numerator > 0)
                    .map(|drop| drop.item.clone()),
            );
            if let Some(mechanics) = &combat.mechanics {
                pools.extend(&mechanics.loot);
            }
        }
    }
    while let Some(pool) = pools.pop() {
        match pool {
            LootPool::Guaranteed { items: entries } => {
                items.extend(entries.iter().map(|entry| entry.item.clone()))
            }
            LootPool::Exclusive { entries, .. } => items.extend(
                entries
                    .iter()
                    .filter(|entry| entry.weight > 0)
                    .flat_map(|entry| &entry.items)
                    .map(|entry| entry.item.clone()),
            ),
            LootPool::Independent {
                chance,
                items: entries,
            } if chance.numerator > 0 => {
                items.extend(entries.iter().map(|entry| entry.item.clone()))
            }
            // This is a literal source-domain proof, not an alternative gameplay guard evaluator.
            LootPool::Conditional { guard, .. }
                if content.mechanics.world_members == Some(false) && requires_members(guard) => {}
            LootPool::Conditional { pools: nested, .. } => pools.extend(nested),
            LootPool::Unresolved { .. } => {
                return Err(invalid(
                    "An unbound loot producer cannot prove item inaccessibility.",
                ));
            }
            _ => {}
        }
    }
    loop {
        let mut additions = Vec::new();
        for id in &items {
            let item = content
                .items
                .get(id)
                .ok_or_else(|| invalid("Unknown introduced item."))?;
            additions.extend(
                item.noted_variant
                    .iter()
                    .chain(item.unnoted_variant.iter())
                    .cloned(),
            );
            if let Some(charges) = &item.charges {
                additions.extend([
                    charges.empty_variant.clone(),
                    charges.charged_variant.clone(),
                ]);
            }
            if let Some(ui) = &content.ui {
                for action in ui.item_actions.get(id).into_iter().flatten() {
                    if let ItemUiAction::Drink { replacement, .. }
                    | ItemUiAction::Empty { replacement } = &action.action
                    {
                        additions.push(replacement.clone());
                    }
                }
            }
        }
        let old = items.len();
        items.extend(additions);
        if items.len() == old {
            break;
        }
    }
    Ok((items, instances))
}

fn requires_members(guard: &Guard) -> bool {
    match guard {
        Guard::MembersWorld => true,
        Guard::All { guards } => guards.iter().any(requires_members),
        _ => false,
    }
}

fn collect_interaction<'a>(
    action: &'a InteractionAction,
    stacks: &mut Vec<&'a ItemStack>,
    effects: &mut Vec<&'a Effect>,
) {
    match action {
        InteractionAction::Gather { rule } => {
            stacks.push(&rule.output);
            if let Some(mechanics) = &rule.mechanics {
                stacks.extend(
                    mechanics
                        .alternatives
                        .iter()
                        .map(|alternative| &alternative.output),
                );
            }
        }
        InteractionAction::Effects { effects: nested }
        | InteractionAction::OpenBank {
            before_open: nested,
            ..
        }
        | InteractionAction::OpenShop {
            before_open: nested,
            ..
        } => effects.extend(nested),
        _ => {}
    }
}

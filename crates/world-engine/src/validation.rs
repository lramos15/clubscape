use std::collections::BTreeSet;

use clubscape_game_types::*;
use clubscape_simulation::{bank, equipment, inventory, skills};

use crate::{invalid_content, invalid_state, runtime, unknown};

pub(crate) fn content(content: &GameContent) -> GameResult<()> {
    if content.schema_version != CONTENT_SCHEMA_VERSION || content.revision.is_empty() {
        return Err(invalid_content(
            "A supported schema and nonempty content revision are required.",
        ));
    }
    for (id, skill) in &content.skills {
        if id != &skill.id {
            return Err(invalid_content("Skill key/definition mismatch."));
        }
        skills::validate_definition(skill)?;
    }
    for (id, stage) in &content.tutorial {
        if id != &stage.id {
            return Err(invalid_content("Tutorial key/definition mismatch."));
        }
        for transition in &stage.transitions {
            transition_definition(content, transition)?;
        }
    }
    for (id, quest) in &content.quests {
        if id != &quest.id
            || !quest.journal.contains_key(&quest.initial_stage)
            || !quest.journal.contains_key(&quest.completed_stage)
        {
            return Err(invalid_content(
                "Quest stages/identity are not fully defined.",
            ));
        }
        for transition in &quest.transitions {
            transition_definition(content, transition)?;
        }
    }
    for (id, spawn) in &content.spawns {
        if id != &spawn.id || !content.regions.contains_key(&spawn.region) {
            return Err(invalid_content("Spawn identity/region mismatch."));
        }
        let mut names = BTreeSet::new();
        for interaction in &spawn.interactions {
            if interaction.name.is_empty() || !names.insert(&interaction.name) {
                return Err(invalid_content(
                    "Interaction names must be nonempty and unique per spawn.",
                ));
            }
            guard(content, &interaction.guard, 0)?;
            match &interaction.action {
                InteractionAction::Gather { rule } => {
                    if rule.attempt_ticks == Some(0)
                        || rule.required_level == 0
                        || (rule.respawn_ticks == Some(0)
                            && (rule.depletion.numerator_at_level_1 != 0
                                || rule.depletion.numerator_at_level_99 != 0))
                    {
                        return Err(invalid_content("Gather cadence/level/respawn is invalid."));
                    }
                    rule.success.validate()?;
                    rule.depletion.validate()?;
                    check_item(content, &rule.output.item)?;
                    for item in &rule.tools {
                        check_item(content, item)?;
                    }
                    if !content.skills.contains_key(&rule.skill) {
                        return Err(unknown(format!("Unknown gathering skill {}.", rule.skill)));
                    }
                }
                InteractionAction::Effects { effects } => effects_definition(content, effects, 0)?,
                InteractionAction::Dialogue { dialogue } => {
                    if !content.dialogues.contains_key(dialogue) {
                        return Err(unknown(format!("Unknown dialogue {dialogue}.")));
                    }
                }
                InteractionAction::Production { recipes } => {
                    for recipe in recipes {
                        if !content.recipes.contains_key(recipe) {
                            return Err(unknown(format!("Unknown recipe {recipe}.")));
                        }
                    }
                }
                InteractionAction::Shop { shop } if !content.shops.contains_key(shop) => {
                    return Err(unknown(format!("Unknown shop {shop}.")));
                }
                _ => {}
            }
        }
        if let SpawnKind::Item {
            stack,
            respawn_ticks,
        } = &spawn.kind
        {
            check_item(content, &stack.item)?;
            if *respawn_ticks == 0 {
                return Err(invalid_content(
                    "Source item spawns need a positive respawn cadence.",
                ));
            }
        }
    }
    for (id, recipe) in &content.recipes {
        if id != &recipe.id || recipe.ticks == Some(0) || recipe.inputs.is_empty() {
            return Err(invalid_content(
                "Recipe identity, consumed inputs or cadence is invalid.",
            ));
        }
        recipe.success.validate()?;
        if let Some(rule) = recipe.item_on_target_rule()? {
            guard(content, &rule.guard, 0)?;
            if recipe
                .target_objects
                .iter()
                .any(|object| !content.objects.contains_key(object))
                || content.spawns.values().flat_map(|spawn| &spawn.interactions)
                    .chain(content.mechanics.temporary_objects.values().flat_map(|object| &object.interactions))
                    .any(|interaction| matches!(&interaction.action, InteractionAction::Production { recipes } if recipes.contains(id)))
                || content.ui.as_ref().is_some_and(|ui| {
                    ui.production_interfaces.contains_key(id) || ui.direct_production.contains(id)
                })
            {
                return Err(invalid_content(
                    "Item-on-only recipes require actual objects and cannot mix menu or one-click Production declarations.",
                ));
            }
        }
        for stack in recipe
            .inputs
            .iter()
            .chain(&recipe.outputs)
            .chain(&recipe.failed_outputs)
        {
            check_item(content, &stack.item)?;
        }
        for item in &recipe.tools {
            check_item(content, item)?;
            if recipe.inputs.iter().any(|input| &input.item == item) {
                return Err(invalid_content(
                    "A required, unconsumed recipe tool cannot also be an input.",
                ));
            }
        }
    }
    for (id, dialogue) in &content.dialogues {
        if id != &dialogue.id || dialogue.entry_nodes.is_empty() {
            return Err(invalid_content(
                "Dialogue identity/entry nodes are invalid.",
            ));
        }
        let mut nodes = BTreeSet::new();
        for node in &dialogue.nodes {
            if node.id.is_empty() || !nodes.insert(&node.id) {
                return Err(invalid_content("Dialogue node identities must be unique."));
            }
            guard(content, &node.guard, 0)?;
            let mut choices = BTreeSet::new();
            for choice in &node.choices {
                if choice.id.is_empty() || !choices.insert(&choice.id) {
                    return Err(invalid_content(
                        "Dialogue choice identities must be unique per node.",
                    ));
                }
                guard(content, &choice.guard, 0)?;
                effects_definition(content, &choice.effects, 0)?;
            }
        }
        for node in dialogue.entry_nodes.iter().chain(
            dialogue
                .nodes
                .iter()
                .flat_map(|node| &node.choices)
                .filter_map(|choice| choice.next_node.as_ref()),
        ) {
            if !nodes.contains(node) {
                return Err(unknown(format!(
                    "Dialogue {id} references undefined node {node}."
                )));
            }
        }
    }
    for (id, shop) in &content.shops {
        if id != &shop.id {
            return Err(invalid_content("Shop identity mismatch."));
        }
        check_item(content, &shop.currency)?;
        let mut stock = BTreeSet::new();
        for row in &shop.stock {
            check_item(content, &row.item)?;
            if row.item == shop.currency
                || !stock.insert(&row.item)
                || row.base_stock > MAX_STACK_QUANTITY
                || row.restock_ticks == 0
                || row.buy_price == 0
            {
                return Err(invalid_content(
                    "Shop stock/currency/cadence/price is invalid.",
                ));
            }
        }
    }
    if content
        .initial_state
        .flags
        .keys()
        .any(|key| key.starts_with(runtime::PREFIX))
    {
        return Err(invalid_content(
            "Source flags must not use the engine-reserved namespace.",
        ));
    }
    Ok(())
}

pub(crate) fn character(character: &CharacterState, content: &GameContent) -> GameResult<()> {
    if character.schema_version != GAME_SCHEMA_VERSION || character.run_energy > MAX_RUN_ENERGY {
        return Err(invalid_state("Character schema/run energy is invalid."));
    }
    character.validate_runtime(content)?;
    inventory::validate(&character.inventory, &content.items)?;
    equipment::validate(&character.equipment, content)?;
    bank::validate(&character.bank, &content.items)?;
    if !content.tutorial.contains_key(&character.tutorial_stage) {
        return Err(unknown(format!(
            "Unknown character stage {}.",
            character.tutorial_stage
        )));
    }
    for (id, state) in &character.skills {
        let definition = content
            .skills
            .get(id)
            .ok_or_else(|| unknown(format!("Unknown skill {id}.")))?;
        skills::level_for_xp(definition, state.xp_tenths)?;
    }
    for (id, state) in &character.quests {
        let definition = content
            .quests
            .get(id)
            .ok_or_else(|| unknown(format!("Unknown quest {id}.")))?;
        if !definition.journal.contains_key(&state.stage) {
            return Err(unknown(format!("Undefined quest stage {}.", state.stage)));
        }
    }
    let mut unlocked = BTreeSet::new();
    for interface in &character.interfaces {
        if !content.interfaces.contains_key(interface) {
            return Err(unknown(format!("Unknown unlocked interface {interface}.")));
        }
        if !unlocked.insert(interface) {
            return Err(invalid_state("An interface is unlocked more than once."));
        }
    }
    Ok(())
}

fn transition_definition(content: &GameContent, transition: &ProgressTransition) -> GameResult<()> {
    if !matches!(
        transition.event.as_str(),
        "moved"
            | "interacted"
            | "dialogue_selected"
            | "interface_opened"
            | "gathered"
            | "produced"
            | "equipped"
            | "xp_gained"
            | "hit"
            | "defeated"
            | "died"
            | "recovered"
            | "tutorial_advanced"
            | "quest_advanced"
            | "message"
            | "sound"
            | "animation"
            | "appearance_confirmed"
            | "experience_selected"
            | "interface_closed"
            | "interface_presented"
            | "setting_changed"
            | "inspected"
            | "production_resolved"
            | "combat_resolved"
            | "npc_killed"
            | "spell_resolved"
            | "teleport"
            | "item_transferred"
            | "food_eaten"
            | "prayer_changed"
            | "temporary_object_created"
            | "object_transformed"
            | "counter_changed"
            | "death_occurred"
            | "death_topic_completed"
            | "recovery_completed"
            | "grave_expired"
    ) {
        return Err(invalid_content(format!(
            "Undefined GameEvent kind {}.",
            transition.event
        )));
    }
    guard(content, &transition.guard, 0)?;
    effects_definition(content, &transition.effects, 0)
}

fn guard(content: &GameContent, value: &Guard, depth: usize) -> GameResult<()> {
    if depth > 64 {
        return Err(invalid_content(
            "Guard nesting exceeds the supported bound.",
        ));
    }
    match value {
        Guard::All { guards } | Guard::Any { guards } => {
            for nested in guards {
                guard(content, nested, depth + 1)?;
            }
        }
        Guard::Not { guard: nested } => guard(content, nested, depth + 1)?,
        Guard::TutorialStage { stage } if !content.tutorial.contains_key(stage) => {
            return Err(unknown(format!("Undefined tutorial guard stage {stage}.")));
        }
        Guard::QuestStage { quest, stage } => check_quest_stage(content, quest, stage)?,
        Guard::HasItems { items } => {
            for stack in items {
                check_item(content, &stack.item)?;
            }
        }
        Guard::Equipped { item } => check_item(content, item)?,
        Guard::SkillAtLeast { requirement } => {
            if !content.skills.contains_key(&requirement.skill) || requirement.level == 0 {
                return Err(invalid_content(
                    "Undefined/invalid guard skill requirement.",
                ));
            }
        }
        Guard::InterfaceUnlocked { interface } if !content.interfaces.contains_key(interface) => {
            return Err(unknown(format!("Unknown guard interface {interface}.")));
        }
        _ => {}
    }
    Ok(())
}

fn effects_definition(content: &GameContent, effects: &[Effect], depth: usize) -> GameResult<()> {
    if depth > 64 || effects.len() > 4096 {
        return Err(invalid_content(
            "Effect nesting/collection exceeds the supported bound.",
        ));
    }
    for effect in effects {
        match effect {
            Effect::GiveItems { items } | Effect::TakeItems { items } => {
                for stack in items {
                    check_item(content, &stack.item)?;
                }
            }
            Effect::AwardXp { rewards } => {
                for reward in rewards {
                    if !content.skills.contains_key(&reward.skill) {
                        return Err(unknown(format!(
                            "Unknown XP reward skill {}.",
                            reward.skill
                        )));
                    }
                }
            }
            Effect::SetFlag { name, .. } if name.starts_with(runtime::PREFIX) => {
                return Err(invalid_content(
                    "Source effects cannot write engine-reserved flags.",
                ));
            }
            Effect::UnlockInterface { interface }
                if !content.interfaces.contains_key(interface) =>
            {
                return Err(unknown(format!("Unknown effect interface {interface}.")));
            }
            Effect::SetTutorialStage { stage } if !content.tutorial.contains_key(stage) => {
                return Err(unknown(format!("Unknown effect stage {stage}.")));
            }
            Effect::SetQuestStage { quest, stage } => check_quest_stage(content, quest, stage)?,
            Effect::Travel { region, .. } if !content.regions.contains_key(region) => {
                return Err(unknown(format!("Unknown travel region {region}.")));
            }
            Effect::Conditional {
                guard: condition,
                effects,
            } => {
                guard(content, condition, depth + 1)?;
                effects_definition(content, effects, depth + 1)?;
            }
            _ => {}
        }
    }
    Ok(())
}

fn check_item(content: &GameContent, id: &ItemId) -> GameResult<()> {
    let item = content
        .items
        .get(id)
        .ok_or_else(|| unknown(format!("Unknown item {id}.")))?;
    if &item.id != id {
        return Err(invalid_content(
            "Item definition key does not match its identity.",
        ));
    }
    Ok(())
}

fn check_quest_stage(content: &GameContent, quest: &QuestId, stage: &StageId) -> GameResult<()> {
    if !content
        .quests
        .get(quest)
        .is_some_and(|quest| quest.journal.contains_key(stage))
    {
        return Err(unknown(format!(
            "Undefined stage {stage} in quest {quest}."
        )));
    }
    Ok(())
}

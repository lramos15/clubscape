use super::*;

pub(super) struct Scan {
    pub interfaces: BTreeSet<InterfaceId>,
    pub mutable_flags: BTreeSet<String>,
}

pub(crate) fn discard_rules(mut content: GameContent) {
    // A rejected, programmatically constructed tree can exceed Rust's recursive
    // drop stack even though the validator never recurses into it.
    let mut guards = Vec::new();
    let mut effects = Vec::new();
    for spawn in content.spawns.values_mut() {
        for interaction in &mut spawn.interactions {
            guards.push(std::mem::replace(&mut interaction.guard, Guard::Always));
            if let InteractionAction::Effects { effects: batch } = &mut interaction.action {
                effects.extend(std::mem::take(batch));
            }
        }
    }
    for dialogue in content.dialogues.values_mut() {
        for node in &mut dialogue.nodes {
            guards.push(std::mem::replace(&mut node.guard, Guard::Always));
            for choice in &mut node.choices {
                guards.push(std::mem::replace(&mut choice.guard, Guard::Always));
                effects.extend(std::mem::take(&mut choice.effects));
            }
        }
    }
    for transitions in content
        .tutorial
        .values_mut()
        .map(|stage| &mut stage.transitions)
        .chain(
            content
                .quests
                .values_mut()
                .map(|quest| &mut quest.transitions),
        )
    {
        for transition in transitions {
            guards.push(std::mem::replace(&mut transition.guard, Guard::Always));
            effects.extend(std::mem::take(&mut transition.effects));
        }
    }
    while let Some(effect) = effects.pop() {
        if let Effect::Conditional {
            guard,
            effects: batch,
        } = effect
        {
            guards.push(guard);
            effects.extend(batch);
        }
    }
    while let Some(guard) = guards.pop() {
        match guard {
            Guard::All { guards: batch } | Guard::Any { guards: batch } => guards.extend(batch),
            Guard::Not { guard } => guards.push(*guard),
            _ => {}
        }
    }
}

enum Work<'a> {
    Guard(&'a Guard, usize),
    Effect(&'a Effect, usize),
}

/// Iterative preflight runs before any recursive rule processing or serialization.
pub(super) fn scan(content: &GameContent) -> GameResult<Scan> {
    let mut pending = Vec::new();
    for spawn in content.spawns.values() {
        bounded(spawn.interactions.len(), "interactions")?;
        for interaction in &spawn.interactions {
            pending.push(Work::Guard(&interaction.guard, 1));
            if let InteractionAction::Effects { effects } = &interaction.action {
                push_effects(&mut pending, effects, 1)?;
            }
        }
    }
    for dialogue in content.dialogues.values() {
        bounded(dialogue.nodes.len(), "dialogue.nodes")?;
        for node in &dialogue.nodes {
            pending.push(Work::Guard(&node.guard, 1));
            bounded(node.choices.len(), "dialogue.choices")?;
            for choice in &node.choices {
                pending.push(Work::Guard(&choice.guard, 1));
                push_effects(&mut pending, &choice.effects, 1)?;
            }
        }
    }
    for transitions in content
        .tutorial
        .values()
        .map(|stage| &stage.transitions)
        .chain(content.quests.values().map(|quest| &quest.transitions))
    {
        bounded(transitions.len(), "transitions")?;
        for transition in transitions {
            pending.push(Work::Guard(&transition.guard, 1));
            push_effects(&mut pending, &transition.effects, 1)?;
        }
    }
    let mut interfaces: BTreeSet<_> = content.initial_state.interfaces.iter().cloned().collect();
    let mut mutable_flags = BTreeSet::new();
    let mut nodes = 0;
    while let Some(work) = pending.pop() {
        nodes += 1;
        if nodes > MAX_RULE_NODES || pending.len() > MAX_RULE_NODES {
            return Err(invalid("rules", "guard/effect node budget exceeded"));
        }
        let depth = match work {
            Work::Guard(_, depth) | Work::Effect(_, depth) => depth,
        };
        if depth > MAX_RULE_DEPTH {
            return Err(invalid("rules", "guard/effect nesting depth exceeds 32"));
        }
        match work {
            Work::Guard(Guard::All { guards } | Guard::Any { guards }, _) => {
                bounded(guards.len(), "guards")?;
                pending.extend(guards.iter().map(|guard| Work::Guard(guard, depth + 1)));
            }
            Work::Guard(Guard::Not { guard }, _) => pending.push(Work::Guard(guard, depth + 1)),
            Work::Effect(Effect::Conditional { guard, effects }, _) => {
                pending.push(Work::Guard(guard, depth + 1));
                push_effects(&mut pending, effects, depth + 1)?;
            }
            Work::Effect(Effect::UnlockInterface { interface }, _) => {
                interfaces.insert(interface.clone());
            }
            Work::Effect(Effect::SetFlag { name, .. }, _) => {
                mutable_flags.insert(name.clone());
            }
            _ => {}
        }
    }
    Ok(Scan {
        interfaces,
        mutable_flags,
    })
}

fn push_effects<'a>(
    pending: &mut Vec<Work<'a>>,
    effects: &'a [Effect],
    depth: usize,
) -> GameResult<()> {
    bounded(effects.len(), "effects")?;
    pending.extend(effects.iter().map(|effect| Work::Effect(effect, depth)));
    if pending.len() > MAX_RULE_NODES {
        return Err(invalid("rules", "guard/effect node budget exceeded"));
    }
    Ok(())
}

impl Validator<'_> {
    pub(super) fn guard(&self, guard: &Guard, path: &str) -> GameResult<()> {
        match guard {
            Guard::Always => {}
            Guard::All { guards } | Guard::Any { guards } => {
                nonempty(guards.len(), path)?;
                for (index, guard) in guards.iter().enumerate() {
                    self.guard(guard, &format!("{path}.guards[{index}]"))?;
                }
            }
            Guard::Not { guard } => self.guard(guard, &format!("{path}.not"))?,
            Guard::Flag { name, .. } => self.flag(name, path)?,
            Guard::TutorialStage { stage } => self.tutorial_stage(stage, path)?,
            Guard::QuestStage { quest, stage } => self.quest_stage(quest, stage, path)?,
            Guard::HasItems { items } => {
                nonempty(items.len(), path)?;
                self.stacks(items, path, true)?;
            }
            Guard::Equipped { item } => {
                if self.item(item, path)?.equipment.is_none() {
                    return Err(invalid(path, format!("{item} cannot be equipped")));
                }
            }
            Guard::SkillAtLeast { requirement } => self.requirement(requirement, path)?,
            Guard::InterfaceUnlocked { interface } => self.interface(interface, path)?,
            Guard::Within { tile, .. } => {
                if !self.collision.contains_key(tile) {
                    return Err(invalid(
                        path,
                        "location guard has no explicit collision cell",
                    ));
                }
            }
        }
        Ok(())
    }

    pub(super) fn effects(&self, effects: &[Effect], path: &str) -> GameResult<()> {
        self.effect_definitions(effects, path)?;
        let mut items = BTreeMap::<(&ItemId, bool), u64>::new();
        let mut xp = BTreeMap::<&SkillId, u64>::new();
        let mut points = 0_u64;
        let mut pending = vec![effects];
        while let Some(effects) = pending.pop() {
            for effect in effects {
                match effect {
                    Effect::GiveItems { items: stacks } | Effect::TakeItems { items: stacks } => {
                        let grant = matches!(effect, Effect::GiveItems { .. });
                        for stack in stacks {
                            let total = items.entry((&stack.item, grant)).or_default();
                            *total += u64::from(stack.quantity.get());
                            if *total > u64::from(MAX_STACK_QUANTITY) {
                                return Err(invalid(
                                    path,
                                    "an effect batch can overflow the item quantity limit",
                                ));
                            }
                        }
                    }
                    Effect::AwardXp { rewards } => {
                        for reward in rewards {
                            let total = xp.entry(&reward.skill).or_default();
                            *total = total
                                .checked_add(reward.amount_tenths)
                                .ok_or_else(|| invalid(path, "XP effect batch overflows u64"))?;
                            if *total > self.content.skills[&reward.skill].maximum_xp_tenths {
                                return Err(invalid(
                                    path,
                                    "XP effect batch exceeds the skill maximum XP",
                                ));
                            }
                        }
                    }
                    Effect::AddQuestPoints { amount } => {
                        points += u64::from(*amount);
                        if points > u64::from(u32::MAX) {
                            return Err(invalid(path, "quest-point effect batch overflows u32"));
                        }
                    }
                    Effect::Conditional { effects, .. } => pending.push(effects),
                    _ => {}
                }
            }
        }
        Ok(())
    }

    fn effect_definitions(&self, effects: &[Effect], path: &str) -> GameResult<()> {
        for (index, effect) in effects.iter().enumerate() {
            let path = format!("{path}.effects[{index}]");
            match effect {
                Effect::GiveItems { items } | Effect::TakeItems { items } => {
                    nonempty(items.len(), &path)?;
                    self.stacks(items, &path, true)?;
                }
                Effect::AwardXp { rewards } => {
                    nonempty(rewards.len(), &path)?;
                    self.xp(rewards, &path)?;
                }
                Effect::SetFlag { name, .. } => self.flag(name, &path)?,
                Effect::UnlockInterface { interface } => self.interface(interface, &path)?,
                Effect::SetTutorialStage { stage } => self.tutorial_stage(stage, &path)?,
                Effect::SetQuestStage { quest, stage } => self.quest_stage(quest, stage, &path)?,
                Effect::AddQuestPoints { amount } => {
                    if *amount == 0 {
                        return Err(invalid(&path, "quest-point award must be positive"));
                    }
                }
                Effect::Travel { region, tile } => self.location(region, *tile, true, &path)?,
                Effect::Message { text: message } => text(message, &path, MAX_TEXT_BYTES)?,
                Effect::Conditional { guard, effects } => {
                    self.guard(guard, &format!("{path}.guard"))?;
                    nonempty(effects.len(), &path)?;
                    self.effect_definitions(effects, &path)?;
                }
            }
        }
        Ok(())
    }

    fn flag(&self, name: &str, path: &str) -> GameResult<()> {
        token(name, path)?;
        if !self.content.initial_state.flags.contains_key(name) {
            return Err(invalid(
                path,
                format!("flag {name} has no explicit initial value"),
            ));
        }
        Ok(())
    }

    pub(super) fn transition(&self, transition: &ProgressTransition, path: &str) -> GameResult<()> {
        self.guard(&transition.guard, &format!("{path}.guard"))?;
        nonempty(transition.effects.len(), &format!("{path}.effects"))?;
        self.effects(&transition.effects, path)?;
        let target = transition.target.as_deref();
        macro_rules! target {
            ($id:ident, $map:ident) => {
                if let Some(target) = target {
                    let id = $id::new(target).map_err(|error| invalid(path, error))?;
                    if !self.content.$map.contains_key(&id) {
                        return Err(invalid(path, format!("undefined event target {id}")));
                    }
                }
            };
        }
        match transition.event.as_str() {
            "interacted" | "dialogue_selected" | "gathered" | "hit" | "defeated" => {
                target!(SpawnId, spawns);
                if let Some(target) = target {
                    let spawn = &self.content.spawns
                        [&SpawnId::new(target).map_err(|error| invalid(path, error))?];
                    match transition.event.as_str() {
                        "hit" | "defeated" => {
                            if !matches!(&spawn.kind, SpawnKind::Npc { npc } if self.content.npcs[npc].combat.is_some())
                            {
                                return Err(invalid(
                                    path,
                                    "combat event target is not a combat NPC",
                                ));
                            }
                        }
                        "gathered" => {
                            if !spawn.interactions.iter().any(|interaction| {
                                matches!(interaction.action, InteractionAction::Gather { .. })
                            }) {
                                return Err(invalid(
                                    path,
                                    "gathered event target has no gathering action",
                                ));
                            }
                        }
                        "dialogue_selected"
                            if !spawn.interactions.iter().any(|interaction| {
                                matches!(interaction.action, InteractionAction::Dialogue { .. })
                            }) =>
                        {
                            return Err(invalid(
                                path,
                                "dialogue event target has no dialogue action",
                            ));
                        }
                        _ => {}
                    }
                }
            }
            "produced" => {
                target!(RecipeId, recipes);
            }
            "xp_gained" => {
                target!(SkillId, skills);
            }
            "tutorial_advanced" => {
                target!(StageId, tutorial);
            }
            "quest_advanced" => {
                target!(QuestId, quests);
            }
            "equipped" => {
                if let Some(target) = target {
                    let slot = SlotId::new(target).map_err(|error| invalid(path, error))?;
                    if !self.slots.contains(&slot) {
                        return Err(invalid(path, format!("undefined equipped slot {slot}")));
                    }
                }
            }
            "interface_opened" => {
                if let Some(target) = target {
                    self.interface(
                        &InterfaceId::new(target).map_err(|error| invalid(path, error))?,
                        path,
                    )?;
                }
            }
            "moved" | "died" | "recovered" => {
                if target.is_some() {
                    return Err(invalid(
                        path,
                        "this event has no string target; use a guard to qualify it",
                    ));
                }
            }
            _ => {
                return Err(invalid(
                    path,
                    format!(
                        "unknown or unsupported progression event {:?}",
                        transition.event
                    ),
                ));
            }
        }
        Ok(())
    }
}

pub(super) const ACTIONS: &[&str] = &[
    "walk",
    "interact",
    "select_dialogue",
    "open_interface",
    "close_interface",
    "equip",
    "unequip",
    "drop",
    "take_ground_item",
    "use_item",
    "move_inventory",
    "eat",
    "produce",
    "bank_deposit",
    "bank_withdraw",
    "shop_buy",
    "shop_sell",
    "set_combat_style",
    "cast",
    "set_prayer",
    "cancel_activity",
    "request_logout",
];

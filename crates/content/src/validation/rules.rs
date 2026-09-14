use super::*;

pub(super) struct Scan {
    pub mutable_flags: BTreeSet<String>,
}

pub(crate) fn discard_rules(mut content: GameContent) {
    // A rejected, programmatically constructed tree can exceed Rust's recursive
    // drop stack even though the validator never recurses into it.
    let mut guards = Vec::new();
    let mut effects = Vec::new();
    let mut loot = Vec::new();
    for traversal in content.mechanics.traversal.values_mut() {
        guards.push(std::mem::replace(&mut traversal.guard, Guard::Always));
    }
    for spawn in content.spawns.values_mut() {
        for interaction in &mut spawn.interactions {
            guards.push(std::mem::replace(&mut interaction.guard, Guard::Always));
            match &mut interaction.action {
                InteractionAction::Effects { effects: batch }
                | InteractionAction::OpenBank {
                    before_open: batch, ..
                }
                | InteractionAction::OpenShop {
                    before_open: batch, ..
                } => {
                    effects.extend(std::mem::take(batch));
                }
                _ => {}
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
    for recipe in content.recipes.values_mut() {
        if let Some(mechanics) = &mut recipe.mechanics {
            guards.push(std::mem::replace(&mut mechanics.guard, Guard::Always));
            effects.extend(std::mem::take(&mut mechanics.success_effects));
            effects.extend(std::mem::take(&mut mechanics.failure_effects));
        }
    }
    for travel in content.mechanics.travels.values_mut() {
        guards.push(std::mem::replace(&mut travel.guard, Guard::Always));
        effects.extend(std::mem::take(&mut travel.completion_effects));
    }
    for object in content.mechanics.temporary_objects.values_mut() {
        guards.push(std::mem::replace(
            &mut object.placement_guard,
            Guard::Always,
        ));
        for interaction in &mut object.interactions {
            guards.push(std::mem::replace(&mut interaction.guard, Guard::Always));
            if let InteractionAction::Effects { effects: batch }
            | InteractionAction::OpenBank {
                before_open: batch, ..
            }
            | InteractionAction::OpenShop {
                before_open: batch, ..
            } = &mut interaction.action
            {
                effects.extend(std::mem::take(batch));
            }
        }
    }
    for spell in content.mechanics.spells.values_mut() {
        guards.push(std::mem::replace(&mut spell.guard, Guard::Always));
    }
    for experience in content.mechanics.experiences.values_mut() {
        guards.push(std::mem::replace(
            &mut experience.selection_guard,
            Guard::Always,
        ));
    }
    if let Some(appearance) = &mut content.mechanics.appearance {
        guards.push(std::mem::replace(
            &mut appearance.confirmation_guard,
            Guard::Always,
        ));
    }
    for npc in content.npcs.values_mut() {
        if let Some(mechanics) = npc
            .combat
            .as_mut()
            .and_then(|combat| combat.mechanics.as_mut())
        {
            loot.extend(std::mem::take(&mut mechanics.loot));
            if let SourceBinding::Bound { value, .. } = &mut mechanics.eligibility {
                for rule in value {
                    guards.push(std::mem::replace(&mut rule.guard, Guard::Always));
                }
            }
            if let SourceBinding::Bound { value, .. } = &mut mechanics.engagement
                && let Some(aggression) = &mut value.aggression
            {
                guards.push(std::mem::replace(&mut aggression.guard, Guard::Always));
            }
        }
    }
    while let Some(pool) = loot.pop() {
        if let LootPool::Conditional { guard, pools } = pool {
            guards.push(guard);
            loot.extend(pools);
        }
    }
    while let Some(effect) = effects.pop() {
        match effect {
            Effect::Conditional {
                guard,
                effects: batch,
            } => {
                guards.push(guard);
                effects.extend(batch);
            }
            Effect::Once { effects: batch, .. } => effects.extend(batch),
            _ => {}
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
    Loot(&'a LootPool, usize),
}

/// Iterative preflight runs before any recursive rule processing or serialization.
pub(super) fn scan(content: &GameContent) -> GameResult<Scan> {
    let mut pending = Vec::new();
    for traversal in content.mechanics.traversal.values() {
        pending.push(Work::Guard(&traversal.guard, 1));
    }
    for spawn in content.spawns.values() {
        bounded(spawn.interactions.len(), "interactions")?;
        for interaction in &spawn.interactions {
            pending.push(Work::Guard(&interaction.guard, 1));
            match &interaction.action {
                InteractionAction::Effects { effects }
                | InteractionAction::OpenBank {
                    before_open: effects,
                    ..
                }
                | InteractionAction::OpenShop {
                    before_open: effects,
                    ..
                } => push_effects(&mut pending, effects, 1)?,
                _ => {}
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
    for recipe in content.recipes.values() {
        if let Some(mechanics) = &recipe.mechanics {
            pending.push(Work::Guard(&mechanics.guard, 1));
            push_effects(&mut pending, &mechanics.success_effects, 1)?;
            push_effects(&mut pending, &mechanics.failure_effects, 1)?;
        }
    }
    for travel in content.mechanics.travels.values() {
        pending.push(Work::Guard(&travel.guard, 1));
        push_effects(&mut pending, &travel.completion_effects, 1)?;
    }
    for object in content.mechanics.temporary_objects.values() {
        pending.push(Work::Guard(&object.placement_guard, 1));
        for interaction in &object.interactions {
            pending.push(Work::Guard(&interaction.guard, 1));
            if let InteractionAction::Effects { effects }
            | InteractionAction::OpenBank {
                before_open: effects,
                ..
            }
            | InteractionAction::OpenShop {
                before_open: effects,
                ..
            } = &interaction.action
            {
                push_effects(&mut pending, effects, 1)?;
            }
        }
    }
    for spell in content.mechanics.spells.values() {
        pending.push(Work::Guard(&spell.guard, 1));
    }
    for experience in content.mechanics.experiences.values() {
        pending.push(Work::Guard(&experience.selection_guard, 1));
    }
    if let Some(appearance) = &content.mechanics.appearance {
        pending.push(Work::Guard(&appearance.confirmation_guard, 1));
    }
    for npc in content.npcs.values() {
        if let Some(mechanics) = npc
            .combat
            .as_ref()
            .and_then(|combat| combat.mechanics.as_ref())
        {
            bounded(mechanics.loot.len(), "loot")?;
            pending.extend(mechanics.loot.iter().map(|pool| Work::Loot(pool, 1)));
            if let SourceBinding::Bound { value, .. } = &mechanics.eligibility {
                bounded(value.len(), "attack_eligibility")?;
                pending.extend(value.iter().map(|rule| Work::Guard(&rule.guard, 1)));
            }
            if let SourceBinding::Bound { value, .. } = &mechanics.engagement
                && let Some(aggression) = &value.aggression
            {
                pending.push(Work::Guard(&aggression.guard, 1));
            }
        }
    }
    let mut mutable_flags = BTreeSet::new();
    let mut claims = BTreeMap::<&EntitlementId, &[Effect]>::new();
    let mut claim_sites = Vec::new();
    let mut nodes = 0;
    while let Some(work) = pending.pop() {
        nodes += 1;
        if nodes > MAX_RULE_NODES || pending.len() > MAX_RULE_NODES {
            return Err(invalid("rules", "guard/effect node budget exceeded"));
        }
        let depth = match work {
            Work::Guard(_, depth) | Work::Effect(_, depth) | Work::Loot(_, depth) => depth,
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
            Work::Effect(
                Effect::Once {
                    entitlement,
                    effects,
                },
                _,
            ) => {
                claim_sites.push((entitlement, effects.as_slice()));
                push_effects(&mut pending, effects, depth + 1)?;
            }
            Work::Loot(LootPool::Conditional { guard, pools }, _) => {
                bounded(pools.len(), "loot")?;
                pending.push(Work::Guard(guard, depth + 1));
                pending.extend(pools.iter().map(|pool| Work::Loot(pool, depth + 1)));
            }
            Work::Effect(Effect::SetFlag { name, .. }, _) => {
                mutable_flags.insert(name.clone());
            }
            _ => {}
        }
    }
    for (entitlement, effects) in claim_sites {
        if claims
            .insert(entitlement, effects)
            .is_some_and(|previous| previous != effects)
        {
            return Err(invalid(
                "rules",
                "one entitlement cannot claim different reward bundles",
            ));
        }
    }
    Ok(Scan { mutable_flags })
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
    pub(super) fn state_guard(&self, guard: &Guard, path: &str) -> GameResult<()> {
        self.guard(guard, path)?;
        let mut pending = vec![guard];
        while let Some(guard) = pending.pop() {
            match guard {
                Guard::Event { .. } => {
                    return Err(invalid(
                        path,
                        "event facts are post-event progression conditions, not request/state guards",
                    ));
                }
                Guard::All { guards } | Guard::Any { guards } => pending.extend(guards),
                Guard::Not { guard } => pending.push(guard),
                _ => {}
            }
        }
        Ok(())
    }

    pub(super) fn state_effects(&self, effects: &[Effect], path: &str) -> GameResult<()> {
        self.effects(effects, path)?;
        let mut pending = vec![effects];
        while let Some(effects) = pending.pop() {
            for effect in effects {
                match effect {
                    Effect::Conditional { guard, effects } => {
                        self.state_guard(guard, path)?;
                        pending.push(effects);
                    }
                    Effect::Once { effects, .. } => pending.push(effects),
                    _ => {}
                }
            }
        }
        Ok(())
    }

    pub(super) fn allowed_action(&self, action: &str, path: &str) -> GameResult<()> {
        if ACTIONS.contains(&action)
            || [
                "*",
                "dialogue",
                "bank",
                "shop",
                "gather",
                "combat_style",
                "prayer",
            ]
            .contains(&action)
        {
            return Ok(());
        }
        if let Some((family, selector)) = action.split_once(':') {
            match family {
                "interact" => {
                    let (spawn, action) = selector
                        .split_once(':')
                        .map_or((selector, None), |(spawn, action)| (spawn, Some(action)));
                    let spawn = self.spawn_definition(
                        &SpawnId::new(spawn).map_err(|error| invalid(path, error))?,
                        path,
                    )?;
                    if action.is_some_and(|action| {
                        !spawn
                            .interactions
                            .iter()
                            .any(|interaction| interaction.name == action)
                    }) {
                        return Err(invalid(path, "allowed interaction name is undefined"));
                    }
                }
                "gather" => {
                    let spawn = self.spawn_definition(
                        &SpawnId::new(selector).map_err(|error| invalid(path, error))?,
                        path,
                    )?;
                    if !spawn.interactions.iter().any(|interaction| {
                        matches!(interaction.action, InteractionAction::Gather { .. })
                    }) {
                        return Err(invalid(
                            path,
                            "allowed gather target has no gathering action",
                        ));
                    }
                }
                "produce" => {
                    let id = RecipeId::new(selector).map_err(|error| invalid(path, error))?;
                    if !self.content.recipes.contains_key(&id) {
                        return Err(invalid(path, "undefined allowed recipe"));
                    }
                }
                "shop" => {
                    let id = ShopId::new(selector).map_err(|error| invalid(path, error))?;
                    if !self.content.shops.contains_key(&id) {
                        return Err(invalid(path, "undefined allowed shop"));
                    }
                }
                "open_interface" => self.interface(
                    &InterfaceId::new(selector).map_err(|error| invalid(path, error))?,
                    path,
                )?,
                "cast" => {
                    let id = SpellId::new(selector).map_err(|error| invalid(path, error))?;
                    if !self.content.mechanics.spells.contains_key(&id) {
                        return Err(invalid(path, "undefined allowed spell"));
                    }
                }
                "prayer" => {
                    let id = PrayerId::new(selector).map_err(|error| invalid(path, error))?;
                    if !self.content.mechanics.prayers.contains_key(&id) {
                        return Err(invalid(path, "undefined allowed prayer"));
                    }
                }
                _ => return Err(invalid(path, format!("unknown allowed action {action:?}"))),
            }
            return Ok(());
        }
        Err(invalid(
            path,
            format!("unknown allowed action {action:?}; use a declared intent/family/selector"),
        ))
    }

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
            Guard::FreeCapacity { container, slots } => {
                let maximum = match container {
                    ContainerKind::Inventory => INVENTORY_SLOTS as u16,
                    ContainerKind::Bank => self.content.initial_state.bank.capacity,
                    _ => {
                        return Err(invalid(
                            path,
                            "free-capacity guard supports inventory or bank",
                        ));
                    }
                };
                if *slots == 0 || *slots > maximum {
                    return Err(invalid(
                        path,
                        "free-capacity requirement exceeds its declared container",
                    ));
                }
            }
            Guard::OwnsItems { items, .. } => {
                nonempty(items.len(), path)?;
                self.stacks(items, path, false)?;
            }
            Guard::Counter { counter, predicate } => {
                let counter = self.counter(counter, path)?;
                match predicate {
                    CounterPredicate::Equals { value } => counter
                        .validate_value(*value)
                        .map_err(|error| invalid(path, error))?,
                    CounterPredicate::IntegerRange { minimum, maximum } => {
                        if minimum > maximum {
                            return Err(invalid(path, "counter predicate has reversed bounds"));
                        }
                        counter
                            .validate_value(CounterValue::Integer(*minimum))
                            .map_err(|error| invalid(path, error))?;
                        counter
                            .validate_value(CounterValue::Integer(*maximum))
                            .map_err(|error| invalid(path, error))?;
                    }
                }
            }
            Guard::EntitlementClaimed { entitlement } => {
                self.entitlement(entitlement, path)?;
            }
            Guard::Event { condition } => self.event_condition(condition, path)?,
            Guard::Experience { experience } => {
                if !self.content.mechanics.experiences.contains_key(experience) {
                    return Err(invalid(path, "undefined experience choice"));
                }
            }
            Guard::Setting { setting } => self.setting(setting, path)?,
            Guard::Life { phase } => {
                if *phase != LifePhase::Alive && self.content.mechanics.death.is_none() {
                    return Err(invalid(path, "death-phase guard requires a death policy"));
                }
            }
            Guard::DeathTopics { topics } => {
                if topics.is_empty()
                    || self
                        .content
                        .mechanics
                        .death
                        .as_ref()
                        .is_none_or(|death| !topics.is_subset(&death.required_topics))
                {
                    return Err(invalid(path, "undefined or empty death-topic guard"));
                }
            }
            Guard::Charges {
                item,
                charge_kind,
                minimum,
            } => self.charge_requirement(item, charge_kind, *minimum, path)?,
            Guard::MembersWorld => {
                if self.content.mechanics.world_members.is_none() {
                    return Err(invalid(
                        path,
                        "members-world predicate requires an explicit world setting",
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
                    Effect::Conditional { effects, .. } | Effect::Once { effects, .. } => {
                        pending.push(effects)
                    }
                    Effect::Grant { grant } => {
                        for line in &self.content.mechanics.grants[grant].lines {
                            let total = items.entry((&line.item, true)).or_default();
                            *total += u64::from(line.quantity.get());
                            if *total > u64::from(MAX_STACK_QUANTITY) {
                                return Err(invalid(
                                    path,
                                    "grant/effect batch can overflow item quantities",
                                ));
                            }
                        }
                    }
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
                Effect::Once {
                    entitlement,
                    effects,
                } => {
                    if self.entitlement(entitlement, &path)?.purpose
                        != EntitlementPurpose::AtomicReward
                    {
                        return Err(invalid(&path, "Once requires an atomic-reward entitlement"));
                    }
                    nonempty(effects.len(), &path)?;
                    self.effect_definitions(effects, &path)?;
                    let mut nested = vec![effects.as_slice()];
                    while let Some(effects) = nested.pop() {
                        for effect in effects {
                            match effect {
                                Effect::Once { .. } => {
                                    return Err(invalid(
                                        &path,
                                        "nested Once claims are ambiguous; use one atomic reward bundle",
                                    ));
                                }
                                Effect::Conditional { effects, .. } => nested.push(effects),
                                Effect::Grant { grant }
                                    if self.content.mechanics.grants[grant].capacity
                                        == CapacityPolicy::OrderedPartial =>
                                {
                                    return Err(invalid(
                                        &path,
                                        "partial grants need their own per-line entitlement, not an atomic Once wrapper",
                                    ));
                                }
                                _ => {}
                            }
                        }
                    }
                }
                Effect::Grant { grant } => {
                    if !self.content.mechanics.grants.contains_key(grant) {
                        return Err(invalid(&path, "undefined item grant"));
                    }
                }
                Effect::RestoreVital { vital, restoration } => {
                    self.restore_vital(*vital, restoration, &path)?
                }
                Effect::SetCounter { counter, value } => {
                    self.counter(counter, &path)?
                        .validate_value(*value)
                        .map_err(|error| invalid(&path, error))?;
                }
                Effect::AddCounter { counter, delta } => {
                    let CounterType::Integer { minimum, maximum } =
                        self.counter(counter, &path)?.value_type
                    else {
                        return Err(invalid(&path, "cannot add to a boolean counter"));
                    };
                    if *delta == 0
                        || i128::from(*delta).abs() > i128::from(maximum) - i128::from(minimum)
                    {
                        return Err(invalid(
                            &path,
                            "counter delta is zero or impossible within declared bounds",
                        ));
                    }
                }
                Effect::TransformObject { transform, state } => {
                    if self
                        .content
                        .mechanics
                        .object_transforms
                        .get(transform)
                        .is_none_or(|definition| !definition.states.contains_key(state))
                    {
                        return Err(invalid(&path, "undefined object transform/state"));
                    }
                }
                Effect::CreateTemporaryObject { definition } => {
                    if !self
                        .content
                        .mechanics
                        .temporary_objects
                        .contains_key(definition)
                    {
                        return Err(invalid(&path, "undefined temporary object"));
                    }
                }
                Effect::TravelVia { travel } => self.travel(travel, &path)?,
                Effect::ReconcileContainers { reconciliation } => {
                    if !self
                        .content
                        .mechanics
                        .reconciliations
                        .contains_key(reconciliation)
                    {
                        return Err(invalid(&path, "undefined container reconciliation"));
                    }
                }
                Effect::CompleteDeathTopic { topic } => {
                    if self
                        .content
                        .mechanics
                        .death
                        .as_ref()
                        .is_none_or(|death| !death.required_topics.contains(topic))
                    {
                        return Err(invalid(&path, "undefined first-death topic"));
                    }
                }
                Effect::ConsumeCharges {
                    item,
                    charge_kind,
                    amount,
                    ..
                } => self.charge_requirement(item, charge_kind, *amount, &path)?,
                Effect::Inspect {
                    target,
                    explanation,
                } => {
                    self.spawn_definition(target, &path)?;
                    token(explanation, &path)?;
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
        self.event_guard_context(
            &transition.guard,
            &transition.event,
            transition.target.as_deref(),
            path,
        )?;
        let mut branches = vec![transition.effects.as_slice()];
        while let Some(effects) = branches.pop() {
            for effect in effects {
                match effect {
                    Effect::Conditional { guard, effects } => {
                        self.event_guard_context(
                            guard,
                            &transition.event,
                            transition.target.as_deref(),
                            path,
                        )?;
                        branches.push(effects);
                    }
                    Effect::Once { effects, .. } => branches.push(effects),
                    _ => {}
                }
            }
        }
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
            "interacted" | "dialogue_selected" | "gathered" | "hit" | "defeated" | "inspected"
            | "combat_resolved" | "npc_killed" => {
                target!(SpawnId, spawns);
                if let Some(target) = target {
                    let spawn = &self.content.spawns
                        [&SpawnId::new(target).map_err(|error| invalid(path, error))?];
                    match transition.event.as_str() {
                        "hit" | "defeated" | "combat_resolved" | "npc_killed" => {
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
            "produced" | "production_resolved" => {
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
            "interface_opened" | "interface_closed" | "interface_presented" => {
                if let Some(target) = target {
                    self.interface(
                        &InterfaceId::new(target).map_err(|error| invalid(path, error))?,
                        path,
                    )?;
                }
            }
            "sound" => {
                if let Some(target) = target {
                    AssetId::new(target).map_err(|error| invalid(path, error))?;
                }
            }
            "animation" => {
                if let Some(target) = target {
                    if target.starts_with("spawn.") {
                        let spawn = SpawnId::new(target).map_err(|error| invalid(path, error))?;
                        if !self.content.spawns.contains_key(&spawn) {
                            return Err(invalid(path, format!("undefined event target {spawn}")));
                        }
                    } else {
                        ActorId::new(target).map_err(|error| invalid(path, error))?;
                    }
                }
            }
            "experience_selected"
            | "spell_resolved"
            | "teleport"
            | "prayer_changed"
            | "temporary_object_created"
            | "object_transformed"
            | "counter_changed" => {
                self.mechanic_event_target(&transition.event, target, path)?;
            }
            "food_eaten" => {
                target!(ItemId, items);
            }
            "moved"
            | "died"
            | "recovered"
            | "message"
            | "appearance_confirmed"
            | "setting_changed"
            | "item_transferred"
            | "death_occurred"
            | "death_topic_completed"
            | "recovery_completed"
            | "grave_expired" => {
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
    "produce_at",
    "produce_selected",
    "interact_with",
    "bank_deposit",
    "bank_withdraw",
    "shop_buy",
    "shop_sell",
    "set_combat_style",
    "cast",
    "set_prayer",
    "set_setting",
    "confirm_appearance",
    "select_experience",
    "reclaim",
    "open_grave",
    "open_death_office",
    "cancel_activity",
    "request_logout",
];
